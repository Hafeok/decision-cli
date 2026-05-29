//! TC-068 — coverage primitive reports covered and uncovered TCs structurally.
//!
//! Validates: FT-045 · ADR-030.
//! Spec: `.product/tests/TC-068-coverage-primitive-reports-covered-and-uncovered-t.md`

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
use decision_cli::core::verify::coverage::{feature_coverage, feature_covered_by};
use decision_cli::vocab::verify_graph_named_graph;
use oxigraph::model::NamedNode;
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
            "decision-cli-tc068-{tag}-{}-{}-{}",
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

/// Seed `.product/features/FT-X-fixture.md` with a YAML frontmatter listing
/// the supplied TCs. Mirrors the product-cli feature shape (FT-045 §Inputs).
fn write_feature_fixture(workdir: &Path, feature_id: &str, tcs: &[&str]) {
    let dir = workdir.join(".product/features");
    fs::create_dir_all(&dir).expect("create features dir");
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str(&format!("id: {feature_id}\n"));
    body.push_str("title: TC-068 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-068.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write fixture");
}

fn insert_quads(store: &Store, quads: &[oxigraph::model::Quad]) {
    for q in quads {
        store.insert(q.as_ref()).expect("insert");
    }
}

/// Build VG-X with two covering steps for T1, T2 (T3 uncovered).
fn build_vg_x() -> VerificationGraph {
    let mut step0 = VerificationStep::new(
        "VG-X",
        0,
        StepFields::ShellCommand {
            command: "echo step1".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    step0.provides_evidence_for = vec![NamedNode::new_unchecked(tc_iri_for("TC-T1"))];

    let mut step1 = VerificationStep::new(
        "VG-X",
        1,
        StepFields::ShellCommand {
            command: "echo step2".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    step1.provides_evidence_for = vec![NamedNode::new_unchecked(tc_iri_for("TC-T2"))];

    VerificationGraph::new(
        "VG-X",
        ArtifactRef(NamedNode::new_unchecked(feature_iri_for("FT-X"))),
        NamedNode::new_unchecked("https://decision-cli.dev/ns/bench/BNCH-001-ephemeral-cli"),
        vec![step0, step1],
    )
}

#[test]
fn tc_068_feature_covered_by_reports_per_graph_partition() {
    let wd = WorkdirGuard::new("per-graph");
    write_feature_fixture(wd.path(), "FT-X", &["TC-T1", "TC-T2", "TC-T3"]);

    let store = Store::new().expect("in-memory store");
    let vg = build_vg_x();
    insert_quads(&store, &vg.to_quads(verify_graph_named_graph()));

    let report =
        feature_covered_by("FT-X", "VG-X", &store, wd.path()).expect("feature_covered_by ok");

    // 1. all_tcs reflects the feature's declared TC list, in order.
    assert_eq!(
        report.all_tcs,
        vec![
            tc_iri_for("TC-T1"),
            tc_iri_for("TC-T2"),
            tc_iri_for("TC-T3"),
        ]
    );

    // 2. considered = [VG-X].
    assert_eq!(report.considered, vec![graph_iri_for("VG-X")]);

    // 3. covered = [(T1, VG-X, step-0), (T2, VG-X, step-1)] — deterministic order.
    assert_eq!(report.covered.len(), 2, "expected two covering hits");
    assert_eq!(report.covered[0].tc, tc_iri_for("TC-T1"));
    assert_eq!(report.covered[0].graph, graph_iri_for("VG-X"));
    assert!(
        report.covered[0].step.ends_with("/VG-X/0"),
        "first hit step: {}",
        report.covered[0].step
    );
    assert_eq!(report.covered[1].tc, tc_iri_for("TC-T2"));
    assert_eq!(report.covered[1].graph, graph_iri_for("VG-X"));
    assert!(
        report.covered[1].step.ends_with("/VG-X/1"),
        "second hit step: {}",
        report.covered[1].step
    );

    // 4. uncovered = [T3].
    assert_eq!(report.uncovered, vec![tc_iri_for("TC-T3")]);

    // 5. feature is the IRI.
    assert_eq!(report.feature, feature_iri_for("FT-X"));
}

#[test]
fn tc_068_feature_coverage_none_candidates_discovers_graphs_in_store() {
    let wd = WorkdirGuard::new("rollup");
    write_feature_fixture(wd.path(), "FT-X", &["TC-T1", "TC-T2", "TC-T3"]);

    let store = Store::new().expect("in-memory store");
    insert_quads(&store, &build_vg_x().to_quads(verify_graph_named_graph()));

    let report = feature_coverage("FT-X", None, &store, wd.path()).expect("feature_coverage ok");

    assert_eq!(report.uncovered, vec![tc_iri_for("TC-T3")]);
    assert_eq!(report.considered, vec![graph_iri_for("VG-X")]);
    assert_eq!(report.covered.len(), 2);
}

#[test]
fn tc_068_primitive_is_deterministic_across_repeat_calls() {
    let wd = WorkdirGuard::new("determinism");
    write_feature_fixture(wd.path(), "FT-X", &["TC-T1", "TC-T2", "TC-T3"]);

    let store = Store::new().expect("in-memory store");
    insert_quads(&store, &build_vg_x().to_quads(verify_graph_named_graph()));

    let r1 = feature_covered_by("FT-X", "VG-X", &store, wd.path()).expect("r1");
    let r2 = feature_covered_by("FT-X", "VG-X", &store, wd.path()).expect("r2");

    assert_eq!(r1, r2, "fixed snapshot must produce byte-equal reports");
}

#[test]
fn tc_068_primitive_performs_no_writes() {
    let wd = WorkdirGuard::new("no-writes");
    write_feature_fixture(wd.path(), "FT-X", &["TC-T1", "TC-T2", "TC-T3"]);

    let store = Store::new().expect("in-memory store");
    insert_quads(&store, &build_vg_x().to_quads(verify_graph_named_graph()));

    let before = store.len().expect("len before");
    let _ = feature_covered_by("FT-X", "VG-X", &store, wd.path()).expect("ok");
    let _ = feature_coverage("FT-X", None, &store, wd.path()).expect("ok");
    let after = store.len().expect("len after");
    assert_eq!(before, after, "primitive must not mutate the store");

    // And no files should be created under workdir other than .product/features.
    let dec_dir = wd.path().join(".dec");
    assert!(!dec_dir.exists(), "primitive must not create .dec/");
}

#[test]
fn tc_068_zero_tc_feature_reports_empty_partition() {
    let wd = WorkdirGuard::new("zero-tcs");
    write_feature_fixture(wd.path(), "FT-EMPTY", &[]);

    let store = Store::new().expect("in-memory store");
    let report = feature_coverage("FT-EMPTY", None, &store, wd.path()).expect("ok");
    assert!(report.all_tcs.is_empty());
    assert!(report.covered.is_empty());
    assert!(report.uncovered.is_empty());
}

#[test]
fn tc_068_empty_candidate_set_yields_full_uncovered() {
    let wd = WorkdirGuard::new("empty-candidates");
    write_feature_fixture(wd.path(), "FT-X", &["TC-T1", "TC-T2", "TC-T3"]);

    let store = Store::new().expect("in-memory store");
    insert_quads(&store, &build_vg_x().to_quads(verify_graph_named_graph()));

    let report = feature_coverage("FT-X", Some(Vec::new()), &store, wd.path())
        .expect("empty candidate set accepted");
    assert!(report.covered.is_empty());
    assert_eq!(
        report.uncovered,
        vec![
            tc_iri_for("TC-T1"),
            tc_iri_for("TC-T2"),
            tc_iri_for("TC-T3"),
        ]
    );
    assert!(report.considered.is_empty());
}

#[test]
fn tc_068_unknown_feature_surfaces_artifact_not_found() {
    let wd = WorkdirGuard::new("unknown-feature");
    // No fixture written — directory exists but no matching md.
    fs::create_dir_all(wd.path().join(".product/features")).expect("mk");

    let store = Store::new().expect("in-memory store");
    let err = feature_coverage("FT-MISSING", None, &store, wd.path()).expect_err("must fail");
    use decision_cli::core::verify::coverage::CoverageError;
    match err {
        CoverageError::ArtifactNotFound { kind, id } => {
            assert_eq!(kind, "Feature");
            assert_eq!(id, "FT-MISSING");
        }
        other => panic!("expected ArtifactNotFound; got {other:?}"),
    }
}

#[test]
fn tc_068_unknown_candidate_graph_surfaces_artifact_not_found() {
    let wd = WorkdirGuard::new("unknown-graph");
    write_feature_fixture(wd.path(), "FT-X", &["TC-T1"]);

    let store = Store::new().expect("in-memory store");
    let err = feature_coverage(
        "FT-X",
        Some(vec!["VG-DOES-NOT-EXIST".to_string()]),
        &store,
        wd.path(),
    )
    .expect_err("must fail");
    use decision_cli::core::verify::coverage::CoverageError;
    match err {
        CoverageError::ArtifactNotFound { kind, id } => {
            assert_eq!(kind, "VerificationGraph");
            assert_eq!(id, "VG-DOES-NOT-EXIST");
        }
        other => panic!("expected ArtifactNotFound; got {other:?}"),
    }
}
