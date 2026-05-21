//! TC-072 — matcher excludes graphs in different environment from match set.
//!
//! Validates: FT-046 · ADR-030.
//! Spec: `.product/tests/TC-072-matcher-excludes-graphs-in-different-environment-f.md`

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
use decision_cli::vocab::{verify_graph_named_graph, IRI_DEC_ENV_PREFIX};
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
            "decision-cli-tc072-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-072 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-072.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write fixture");
}

fn insert_quads(store: &Store, quads: &[Quad]) {
    for q in quads {
        store.insert(q.as_ref()).expect("insert");
    }
}

fn seed_env(store: &Store, env_iri: &str) {
    let verify_env_graph =
        NamedNodeRef::new_unchecked("https://decision-cli.dev/ns/graph/verify-env");
    let env = NamedNode::new_unchecked(env_iri.to_string());
    let rdf_type = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let env_class = NamedNode::new_unchecked("https://decision-cli.dev/ns#VerificationEnvironment");
    let q = QuadRef::new(&env, &rdf_type, &env_class, verify_env_graph);
    store.insert(q).expect("insert env type");
}

fn env_iri(short: &str) -> String {
    format!("{IRI_DEC_ENV_PREFIX}{short}")
}

/// One step per TC. Each step declares `dec:providesEvidenceFor TC`.
fn build_graph(graph_id: &str, env_iri_str: &str, covers: &[&str]) -> VerificationGraph {
    let mut steps = Vec::with_capacity(covers.len());
    for (idx, tc) in covers.iter().enumerate() {
        let mut step = VerificationStep::new(
            graph_id,
            idx,
            StepFields::ShellCommand {
                command: format!("echo {tc}"),
                expect_exit_code: Some(0),
                capture_output: None,
            },
        );
        step.provides_evidence_for = vec![NamedNode::new_unchecked(tc_iri_for(tc))];
        steps.push(step);
    }
    VerificationGraph::new(
        graph_id,
        ArtifactRef(NamedNode::new_unchecked(feature_iri_for("FT-V"))),
        NamedNode::new_unchecked(env_iri_str.to_string()),
        steps,
    )
}

#[test]
fn tc_072_matcher_excludes_graphs_in_different_environment_from_match_set() {
    let wd = WorkdirGuard::new("env-isolation");
    write_feature_fixture(wd.path(), "FT-V", &["T1", "T2"]);

    let env1_iri = env_iri("ENV-1");
    let env2_iri = env_iri("ENV-2");

    let store = Store::new().expect("in-memory store");
    seed_env(&store, &env1_iri);
    seed_env(&store, &env2_iri);

    // VG-A in ENV-1 covers [T1] only.
    insert_quads(
        &store,
        &build_graph("VG-A", &env1_iri, &["T1"]).to_quads(verify_graph_named_graph()),
    );
    // VG-B in ENV-2 covers [T1, T2] — strict superset, but wrong env.
    insert_quads(
        &store,
        &build_graph("VG-B", &env2_iri, &["T1", "T2"]).to_quads(verify_graph_named_graph()),
    );

    let report =
        best_matching_graphs("FT-V", "ENV-1", &store, wd.path()).expect("matcher ok");

    // Acceptance: Partial; only VG-A returned for the ENV-1 query;
    // T2 stays uncovered.
    assert_eq!(report.kind, MatchKind::Partial);
    assert_eq!(report.graphs.len(), 1);
    assert_eq!(report.graphs[0].id, graph_iri_for("VG-A"));
    assert_eq!(report.covered_by_match, vec![tc_iri_for("T1")]);
    assert_eq!(report.residual_uncovered, vec![tc_iri_for("T2")]);

    // VG-B must not appear under any column for the ENV-1 query.
    let serialized = format!("{report:?}");
    assert!(
        !serialized.contains("VG-B"),
        "VG-B (in ENV-2) leaked into the ENV-1 report: {serialized}"
    );
}

#[test]
fn tc_072_querying_env2_returns_vg_b_only() {
    // Cross-check: VG-B is genuinely in ENV-2 (so the exclusion above
    // is about scoping, not absence).
    let wd = WorkdirGuard::new("env2-cross-check");
    write_feature_fixture(wd.path(), "FT-V", &["T1", "T2"]);

    let env1_iri = env_iri("ENV-1");
    let env2_iri = env_iri("ENV-2");
    let store = Store::new().expect("in-memory store");
    seed_env(&store, &env1_iri);
    seed_env(&store, &env2_iri);
    insert_quads(
        &store,
        &build_graph("VG-A", &env1_iri, &["T1"]).to_quads(verify_graph_named_graph()),
    );
    insert_quads(
        &store,
        &build_graph("VG-B", &env2_iri, &["T1", "T2"]).to_quads(verify_graph_named_graph()),
    );

    let r = best_matching_graphs("FT-V", "ENV-2", &store, wd.path()).expect("ok");
    assert_eq!(r.kind, MatchKind::CompleteSingle);
    assert_eq!(r.graphs.len(), 1);
    assert_eq!(r.graphs[0].id, graph_iri_for("VG-B"));
}
