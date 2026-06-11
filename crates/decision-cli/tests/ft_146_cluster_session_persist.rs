//! FT-146 — `persist_cluster_run` lands per-cell `dec:SessionRecord`
//! quads + the parent `dec:ClusterDispatch` activity in the
//! orchestration store, end-to-end through the SHACL chokepoint.
//!
//! Validates: FT-146 · ADR-080 · ADR-050.
//!
//! Scope: the persistence path (the new code FT-146 introduces). The
//! full `cluster_dispatch::run` path is exercised by FT-139's
//! existing TC-373 fixture — this test pins the round-trip from
//! `CellSessionRecord` (in-memory) → SHACL chokepoint → on-disk
//! orchestration store → SPARQL read.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::model::NamedNode;
use oxigraph::sparql::QueryResults;

use decision_cli::core::graph::cluster_session::{
    persist_cluster_run, CellSessionRecord, CellStatus, ClusterOutcome, IRI_DEC_CELL_STATUS,
    IRI_DEC_CLUSTER_DISPATCH, IRI_DEC_CLUSTER_OUTCOME, IRI_DEC_USAGE_SOURCE,
};
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path};
use decision_cli::features::implement::WorkerResponseUsage;

static COUNTER: AtomicU64 = AtomicU64::new(0);

const STREAM_TTL: &str =
    include_str!("../src/core/bundled/assets/streams/engineering-development.ttl");

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-ft146-{tag}-{}-{}-{}",
            std::process::id(),
            nanos,
            counter,
        ));
        fs::create_dir_all(&base).expect("create workdir");
        Self(base)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkdirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bootstrap_workdir(wd: &WorkdirGuard) {
    let seed = wd.path().join("stream.ttl");
    fs::write(&seed, STREAM_TTL).expect("write seed");
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");
}

/// FT-146 TC-A — persistence end-to-end through StreamWriter and SHACL
/// chokepoint, plus SPARQL round-trip. Verifies every field this
/// feature writes lands and is queryable.
#[test]
fn ft_146_cluster_dispatch_persists_session_records_with_token_breakdown() {
    let wd = WorkdirGuard::new("persist");
    bootstrap_workdir(&wd);

    let cluster_iri = NamedNode::new("urn:dec:cluster-dispatch:add-judge-worker/FT-T146a").unwrap();
    let scaleway_cap =
        NamedNode::new("https://decision-cli.dev/ns/capability/qwen3-coder/v1").unwrap();
    let mechanical_cap = NamedNode::new("urn:dec:capability:mechanical").unwrap();

    let started = Utc::now();
    let mid = started + chrono::Duration::milliseconds(150);
    let end = mid + chrono::Duration::milliseconds(220);

    let cells = vec![
        // Mechanical cell — zero tokens, status mechanical, usage unreported.
        CellSessionRecord {
            iri: NamedNode::new(
                "urn:dec:cluster-session:add-judge-worker/FT-T146a/capability_binding",
            )
            .unwrap(),
            capability: mechanical_cap.clone(),
            usage: None,
            status: CellStatus::Mechanical,
            started_at: started,
            ended_at: mid,
        },
        // LLM-backed cell — Scaleway endpoint, cache fields zero, usage
        // worker-reported.
        CellSessionRecord {
            iri: NamedNode::new("urn:dec:cluster-session:add-judge-worker/FT-T146a/agent_loop")
                .unwrap(),
            capability: scaleway_cap.clone(),
            usage: Some(WorkerResponseUsage {
                input_tokens_base: 3210,
                input_tokens_cache_write: 0,
                input_tokens_cache_hit: 0,
                output_tokens: 987,
            }),
            status: CellStatus::Succeeded,
            started_at: mid,
            ended_at: end,
        },
    ];

    persist_cluster_run(
        wd.path(),
        &cluster_iri,
        "FT-T146a",
        "add-judge-worker",
        started,
        end,
        ClusterOutcome::Succeeded,
        &cells,
    )
    .expect("persist_cluster_run must succeed against a bootstrapped workdir");

    // SPARQL round-trip — confirm every load-bearing quad landed.
    let dump = orchestration_dump_path(wd.path());
    let store = load_store_from_dump(&dump).expect("reload store");

    // Cluster activity carries the outcome enum.
    let outcome = ask_literal(&store, &cluster_iri, IRI_DEC_CLUSTER_OUTCOME);
    assert_eq!(outcome.as_deref(), Some("succeeded"));

    let cluster_type_count = count_type(&store, &cluster_iri, IRI_DEC_CLUSTER_DISPATCH);
    assert_eq!(
        cluster_type_count, 1,
        "cluster activity must carry rdf:type dec:ClusterDispatch"
    );

    // Each cell has the four token predicates with the expected values.
    let agent_loop_iri = &cells[1].iri;
    let base = ask_literal(
        &store,
        agent_loop_iri,
        "https://decision-cli.dev/ns#input_tokens_base",
    );
    let cache_write = ask_literal(
        &store,
        agent_loop_iri,
        "https://decision-cli.dev/ns#input_tokens_cache_write",
    );
    let cache_hit = ask_literal(
        &store,
        agent_loop_iri,
        "https://decision-cli.dev/ns#input_tokens_cache_hit",
    );
    let output = ask_literal(
        &store,
        agent_loop_iri,
        "https://decision-cli.dev/ns#output_tokens",
    );
    assert_eq!(base.as_deref(), Some("3210"));
    assert_eq!(cache_write.as_deref(), Some("0"));
    assert_eq!(cache_hit.as_deref(), Some("0"));
    assert_eq!(output.as_deref(), Some("987"));

    // FT-146 framing: usageSource + cellStatus + parent activity link.
    let agent_loop_usage_source = ask_literal(&store, agent_loop_iri, IRI_DEC_USAGE_SOURCE);
    assert_eq!(agent_loop_usage_source.as_deref(), Some("worker-reported"));
    let agent_loop_status = ask_literal(&store, agent_loop_iri, IRI_DEC_CELL_STATUS);
    assert_eq!(agent_loop_status.as_deref(), Some("succeeded"));

    // Mechanical cell records zero tokens + unreported + mechanical.
    let mech_iri = &cells[0].iri;
    assert_eq!(
        ask_literal(
            &store,
            mech_iri,
            "https://decision-cli.dev/ns#input_tokens_base"
        )
        .as_deref(),
        Some("0")
    );
    assert_eq!(
        ask_literal(&store, mech_iri, IRI_DEC_USAGE_SOURCE).as_deref(),
        Some("unreported")
    );
    assert_eq!(
        ask_literal(&store, mech_iri, IRI_DEC_CELL_STATUS).as_deref(),
        Some("mechanical")
    );
}

/// FT-146 TC-B — failing cell still produces a SessionRecord with
/// status=failed. PROV-O coverage stays uniform per FT-146 §Invariants.
#[test]
fn ft_146_failed_cell_still_persists_session_record() {
    let wd = WorkdirGuard::new("failed");
    bootstrap_workdir(&wd);

    let cluster_iri = NamedNode::new("urn:dec:cluster-dispatch:add-judge-worker/FT-T146b").unwrap();
    let cap = NamedNode::new("https://decision-cli.dev/ns/capability/qwen3-coder/v1").unwrap();
    let started = Utc::now();

    let failed_iri =
        NamedNode::new("urn:dec:cluster-session:add-judge-worker/FT-T146b/agent_loop").unwrap();
    let cells = vec![CellSessionRecord {
        iri: failed_iri.clone(),
        capability: cap.clone(),
        usage: None,
        status: CellStatus::Failed,
        started_at: started,
        ended_at: started + chrono::Duration::milliseconds(50),
    }];

    persist_cluster_run(
        wd.path(),
        &cluster_iri,
        "FT-T146b",
        "add-judge-worker",
        started,
        started + chrono::Duration::milliseconds(100),
        ClusterOutcome::CellFailed,
        &cells,
    )
    .expect("persist_cluster_run still writes records on failed cluster");

    let dump = orchestration_dump_path(wd.path());
    let store = load_store_from_dump(&dump).expect("reload store");

    // Status = failed and outcome = cell_failed.
    assert_eq!(
        ask_literal(&store, &failed_iri, IRI_DEC_CELL_STATUS).as_deref(),
        Some("failed")
    );
    assert_eq!(
        ask_literal(&store, &cluster_iri, IRI_DEC_CLUSTER_OUTCOME).as_deref(),
        Some("cell_failed")
    );
    // Tokens default to zero; usageSource is unreported (no usage block).
    assert_eq!(
        ask_literal(&store, &failed_iri, IRI_DEC_USAGE_SOURCE).as_deref(),
        Some("unreported")
    );
}

/// FT-146 TC-C — `prov:wasInformedBy` link from cell session to the
/// parent cluster activity is queryable. Walks the cluster's children
/// via SPARQL — the read path the FT-146 spec promises operators.
#[test]
fn ft_146_cluster_activity_groups_its_cell_sessions() {
    let wd = WorkdirGuard::new("group");
    bootstrap_workdir(&wd);

    let cluster_iri =
        NamedNode::new("urn:dec:cluster-dispatch:add-cli-subcommand/FT-T146c").unwrap();
    let cap = NamedNode::new("https://decision-cli.dev/ns/capability/qwen3-coder/v1").unwrap();
    let now = Utc::now();

    let cells: Vec<CellSessionRecord> = (0..3)
        .map(|i| CellSessionRecord {
            iri: NamedNode::new(format!(
                "urn:dec:cluster-session:add-cli-subcommand/FT-T146c/cell_{i}"
            ))
            .unwrap(),
            capability: cap.clone(),
            usage: Some(WorkerResponseUsage {
                input_tokens_base: 100 * (i + 1) as u64,
                input_tokens_cache_write: 0,
                input_tokens_cache_hit: 0,
                output_tokens: 50 * (i + 1) as u64,
            }),
            status: CellStatus::Succeeded,
            started_at: now,
            ended_at: now,
        })
        .collect();

    persist_cluster_run(
        wd.path(),
        &cluster_iri,
        "FT-T146c",
        "add-cli-subcommand",
        now,
        now,
        ClusterOutcome::Succeeded,
        &cells,
    )
    .expect("persist");

    let dump = orchestration_dump_path(wd.path());
    let store = load_store_from_dump(&dump).expect("reload store");

    // SPARQL: count cells whose prov:wasInformedBy is the cluster.
    // Quads live in the orchestration named graph; query both default
    // graph and any named graph.
    let q = format!(
        "PREFIX prov: <http://www.w3.org/ns/prov#>
         SELECT ?cell WHERE {{
           {{ ?cell prov:wasInformedBy <{cluster}> }}
           UNION
           {{ GRAPH ?g {{ ?cell prov:wasInformedBy <{cluster}> }} }}
         }}",
        cluster = cluster_iri.as_str(),
    );
    let results = store.query(&q).expect("sparql");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions");
    };
    let count = sols.count();
    assert_eq!(
        count, 3,
        "expected 3 cell sessions linked to the cluster activity"
    );

    // SPARQL aggregate: sum of input_tokens_base across the cluster.
    let q = format!(
        "PREFIX prov: <http://www.w3.org/ns/prov#>
         PREFIX dec: <https://decision-cli.dev/ns#>
         PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
         SELECT (SUM(xsd:integer(?base)) AS ?total) WHERE {{
           {{ ?cell prov:wasInformedBy <{cluster}> ;
                    dec:input_tokens_base ?base }}
           UNION
           {{ GRAPH ?g {{ ?cell prov:wasInformedBy <{cluster}> ;
                          dec:input_tokens_base ?base }} }}
         }}",
        cluster = cluster_iri.as_str(),
    );
    let results = store.query(&q).expect("sparql sum");
    let QueryResults::Solutions(mut sols) = results else {
        panic!("expected solutions");
    };
    let first = sols.next().expect("at least one row").expect("solution ok");
    let total_term = first.get("total").expect("?total bound");
    let total_str = match total_term {
        oxigraph::model::Term::Literal(lit) => lit.value().to_string(),
        other => panic!("expected literal total, got {other:?}"),
    };
    // 100 + 200 + 300 = 600
    assert_eq!(total_str, "600", "SPARQL aggregate across cells matches");
}

// --------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------

fn ask_literal(
    store: &oxigraph::store::Store,
    subject: &NamedNode,
    predicate: &str,
) -> Option<String> {
    let q = format!(
        "SELECT ?o WHERE {{ {{ <{s}> <{p}> ?o }} UNION {{ GRAPH ?g {{ <{s}> <{p}> ?o }} }} }}",
        s = subject.as_str(),
        p = predicate,
    );
    let QueryResults::Solutions(sols) = store.query(&q).ok()? else {
        return None;
    };
    for sol in sols {
        let sol = sol.ok()?;
        if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("o") {
            return Some(lit.value().to_string());
        }
    }
    None
}

fn count_type(store: &oxigraph::store::Store, subject: &NamedNode, class: &str) -> usize {
    let q = format!(
        "SELECT ?s WHERE {{ {{ <{s}> a <{c}> }} UNION {{ GRAPH ?g {{ <{s}> a <{c}> }} }} }}",
        s = subject.as_str(),
        c = class,
    );
    let QueryResults::Solutions(sols) = store.query(&q).unwrap() else {
        return 0;
    };
    sols.count()
}
