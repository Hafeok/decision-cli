//! TC-070 — matcher returns CompleteSingle when one graph covers all feature TCs in env.
//!
//! Validates: FT-046 · ADR-030.
//! Spec: `.product/tests/TC-070-matcher-returns-completesingle-when-one-graph-cove.md`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::ontology::verification_graph::{
    ArtifactRef, StepFields, VerificationGraph, VerificationStep,
};
use decision_cli::core::verify::coverage::feature_resolver::{
    feature_iri_for, graph_iri_for, tc_iri_for,
};
use decision_cli::core::verify::matcher::{best_matching_graphs, MatchKind};
use decision_cli::vocab::{verify_graph_named_graph, IRI_DEC_BENCH_PREFIX};
use oxigraph::model::{NamedNode, NamedNodeRef, Quad, QuadRef};
use oxigraph::store::Store;

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc070-{tag}-{}-{}-{}",
            std::process::id(),
            nanos,
            counter,
        ));
        fs::create_dir_all(&base).expect("create temp workdir");
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

fn write_feature_fixture(workdir: &Path, feature_id: &str, tcs: &[&str]) {
    let dir = workdir.join(".product/features");
    fs::create_dir_all(&dir).expect("create features dir");
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str(&format!("id: {feature_id}\n"));
    body.push_str("title: TC-070 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-070.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write fixture");
}

fn write_graph_to_disk(workdir: &Path, graph: &VerificationGraph) {
    use decision_cli::core::ontology::verification_graph::to_canonical_turtle;
    let dir = workdir.join(".dec/verify/graph");
    fs::create_dir_all(&dir).expect("create graph dir");
    let graph_short = graph.id.as_str().rsplit('/').next().unwrap();
    let ttl = to_canonical_turtle(graph);
    fs::write(dir.join(format!("{graph_short}.ttl")), ttl).expect("write graph");
}

fn insert_quads(store: &Store, quads: &[Quad]) {
    for q in quads {
        store.insert(q.as_ref()).expect("insert");
    }
}

/// Register an env IRI as a `dec:VerificationBench` in the
/// verify-bench named graph so the matcher's env-existence check
/// returns true. We do not need full env shape here; the matcher
/// only ASKs for the rdf:type triple.
fn seed_env(store: &Store, env_iri: &str) {
    let verify_bench_graph =
        NamedNodeRef::new_unchecked("https://decision-cli.dev/ns/graph/verify-bench");
    let env = NamedNode::new_unchecked(env_iri.to_string());
    let rdf_type = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let env_class = NamedNode::new_unchecked("https://decision-cli.dev/ns#VerificationBench");
    let q = QuadRef::new(&env, &rdf_type, &env_class, verify_bench_graph);
    store.insert(q).expect("insert env type");
}

fn env_iri(short: &str) -> String {
    format!("{IRI_DEC_BENCH_PREFIX}{short}")
}

/// Build VG-Z with three covering steps for T1, T2, T3.
fn build_vg_z_complete(env_iri_str: &str) -> VerificationGraph {
    let mut s0 = VerificationStep::new(
        "VG-Z",
        0,
        StepFields::ShellCommand {
            command: "true".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    s0.provides_evidence_for = vec![NamedNode::new_unchecked(tc_iri_for("T1"))];

    let mut s1 = VerificationStep::new(
        "VG-Z",
        1,
        StepFields::ShellCommand {
            command: "true".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    s1.provides_evidence_for = vec![NamedNode::new_unchecked(tc_iri_for("T2"))];

    let mut s2 = VerificationStep::new(
        "VG-Z",
        2,
        StepFields::ShellCommand {
            command: "true".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    s2.provides_evidence_for = vec![NamedNode::new_unchecked(tc_iri_for("T3"))];

    VerificationGraph::new(
        "VG-Z",
        ArtifactRef(NamedNode::new_unchecked(feature_iri_for("FT-Z"))),
        NamedNode::new_unchecked(env_iri_str.to_string()),
        vec![s0, s1, s2],
    )
}

/// Build VG-Z2 with a single step covering only T1 — a strict subset.
fn build_vg_z2_partial(env_iri_str: &str) -> VerificationGraph {
    let mut s0 = VerificationStep::new(
        "VG-Z2",
        0,
        StepFields::ShellCommand {
            command: "true".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    s0.provides_evidence_for = vec![NamedNode::new_unchecked(tc_iri_for("T1"))];
    VerificationGraph::new(
        "VG-Z2",
        ArtifactRef(NamedNode::new_unchecked(feature_iri_for("FT-Z"))),
        NamedNode::new_unchecked(env_iri_str.to_string()),
        vec![s0],
    )
}

#[test]
fn tc_070_matcher_returns_completesingle_when_one_graph_covers_all() {
    let wd = WorkdirGuard::new("complete-single");
    write_feature_fixture(wd.path(), "FT-Z", &["T1", "T2", "T3"]);

    let env_short = "ENV-1";
    let env_iri_full = env_iri(env_short);

    let store = Store::new().expect("in-memory store");
    seed_env(&store, &env_iri_full);

    // Build and persist the complete graph
    let vg_complete = build_vg_z_complete(&env_iri_full);
    insert_quads(&store, &vg_complete.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg_complete);

    // Build and persist the partial graph
    let vg_partial = build_vg_z2_partial(&env_iri_full);
    insert_quads(&store, &vg_partial.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg_partial);

    let before = store.len().expect("len before");
    let report = best_matching_graphs("FT-Z", env_short, &store, wd.path()).expect("matcher ok");

    // Acceptance criteria from TC-070:
    assert_eq!(report.kind, MatchKind::CompleteSingle);
    assert_eq!(
        report.graphs.len(),
        1,
        "only the complete graph is returned"
    );
    assert_eq!(report.graphs[0].id, graph_iri_for("VG-Z"));
    assert_eq!(
        report.covered_by_match,
        vec![tc_iri_for("T1"), tc_iri_for("T2"), tc_iri_for("T3"),],
    );
    assert!(report.residual_uncovered.is_empty());

    // Matcher must be side-effect-free (does not mutate the store).
    let after = store.len().expect("len after");
    assert_eq!(before, after, "matcher must not mutate the store");
    // Note: .dec/ now exists because we write graph .ttl files before
    // calling the matcher (required for disk-authoritative cross-check).

    // feature / environment carried as IRIs.
    assert_eq!(report.feature, feature_iri_for("FT-Z"));
    assert_eq!(report.environment, env_iri_full);
}

#[test]
fn tc_070_matcher_is_deterministic_across_repeats() {
    let wd = WorkdirGuard::new("determinism");
    write_feature_fixture(wd.path(), "FT-Z", &["T1", "T2", "T3"]);

    let env_short = "ENV-1";
    let env_iri_full = env_iri(env_short);

    let store = Store::new().expect("in-memory store");
    seed_env(&store, &env_iri_full);

    let vg_complete = build_vg_z_complete(&env_iri_full);
    insert_quads(&store, &vg_complete.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg_complete);

    let vg_partial = build_vg_z2_partial(&env_iri_full);
    insert_quads(&store, &vg_partial.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg_partial);

    let r1 = best_matching_graphs("FT-Z", env_short, &store, wd.path()).expect("r1");
    let r2 = best_matching_graphs("FT-Z", env_short, &store, wd.path()).expect("r2");
    assert_eq!(r1, r2);
}
