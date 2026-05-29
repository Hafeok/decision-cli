//! TC-071 — matcher returns CompleteMultiple deterministic greedy cover with stable tiebreak.
//!
//! Validates: FT-046 · ADR-030.
//! Spec: `.product/tests/TC-071-matcher-returns-completemultiple-deterministic-gre.md`

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
            "decision-cli-tc071-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-071 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-071.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write fixture");
}

fn insert_quads(store: &Store, quads: &[Quad]) {
    for q in quads {
        store.insert(q.as_ref()).expect("insert");
    }
}

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

/// Build a graph with one step per TC it covers, all in `env_iri`.
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
        ArtifactRef(NamedNode::new_unchecked(feature_iri_for("FT-W"))),
        NamedNode::new_unchecked(env_iri_str.to_string()),
        steps,
    )
}

fn write_graph_to_disk(workdir: &Path, graph: &VerificationGraph) {
    use decision_cli::core::ontology::verification_graph::to_canonical_turtle;
    let dir = workdir.join(".dec/verify/graph");
    fs::create_dir_all(&dir).expect("create graph dir");
    let graph_short = graph.id.as_str().rsplit('/').next().unwrap();
    let ttl = to_canonical_turtle(graph);
    fs::write(dir.join(format!("{graph_short}.ttl")), ttl).expect("write graph");
}

#[test]
fn tc_071_matcher_returns_completemultiple_deterministic_greedy_cover() {
    let wd = WorkdirGuard::new("complete-multiple");
    write_feature_fixture(wd.path(), "FT-W", &["T1", "T2", "T3", "T4"]);

    let env_short = "ENV-1";
    let env_iri_full = env_iri(env_short);

    let store = Store::new().expect("in-memory store");
    seed_env(&store, &env_iri_full);

    // VG-3 covers [T1, T2]; VG-5 covers [T2, T3];
    // VG-7 covers [T3, T4]; VG-9 covers [T4].
    let vg3 = build_graph("VG-3", &env_iri_full, &["T1", "T2"]);
    insert_quads(&store, &vg3.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg3);

    let vg5 = build_graph("VG-5", &env_iri_full, &["T2", "T3"]);
    insert_quads(&store, &vg5.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg5);

    let vg7 = build_graph("VG-7", &env_iri_full, &["T3", "T4"]);
    insert_quads(&store, &vg7.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg7);

    let vg9 = build_graph("VG-9", &env_iri_full, &["T4"]);
    insert_quads(&store, &vg9.to_quads(verify_graph_named_graph()));
    write_graph_to_disk(wd.path(), &vg9);

    let report = best_matching_graphs("FT-W", env_short, &store, wd.path()).expect("matcher ok");

    // Acceptance: two-graph cover [VG-3, VG-7], in that order.
    assert_eq!(report.kind, MatchKind::CompleteMultiple);
    let ids: Vec<&str> = report.graphs.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            graph_iri_for("VG-3").as_str(),
            graph_iri_for("VG-7").as_str(),
        ]
    );
    assert_eq!(
        report.covered_by_match,
        vec![
            tc_iri_for("T1"),
            tc_iri_for("T2"),
            tc_iri_for("T3"),
            tc_iri_for("T4"),
        ]
    );
    assert!(report.residual_uncovered.is_empty());

    // Per-graph `covers` reflects the slice each graph contributes,
    // in feature-declaration order.
    assert_eq!(
        report.graphs[0].covers,
        vec![tc_iri_for("T1"), tc_iri_for("T2")]
    );
    assert_eq!(
        report.graphs[1].covers,
        vec![tc_iri_for("T3"), tc_iri_for("T4")]
    );
}

#[test]
fn tc_071_matcher_is_deterministic_across_100_runs() {
    let wd = WorkdirGuard::new("100-runs");
    write_feature_fixture(wd.path(), "FT-W", &["T1", "T2", "T3", "T4"]);

    let env_short = "ENV-1";
    let env_iri_full = env_iri(env_short);

    let store = Store::new().expect("in-memory store");
    seed_env(&store, &env_iri_full);
    for (id, covers) in [
        ("VG-3", &["T1", "T2"][..]),
        ("VG-5", &["T2", "T3"][..]),
        ("VG-7", &["T3", "T4"][..]),
        ("VG-9", &["T4"][..]),
    ] {
        let graph = build_graph(id, &env_iri_full, covers);
        insert_quads(&store, &graph.to_quads(verify_graph_named_graph()));
        write_graph_to_disk(wd.path(), &graph);
    }

    let first = best_matching_graphs("FT-W", env_short, &store, wd.path()).expect("first");
    for _ in 0..99 {
        let r = best_matching_graphs("FT-W", env_short, &store, wd.path()).expect("repeat");
        assert_eq!(r, first, "matcher output must be deterministic");
    }
}
