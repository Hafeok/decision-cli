//! TC-069 — coverage primitive treats absent providesEvidenceFor as no coverage.
//!
//! Validates: FT-045 · ADR-030.
//! Spec: `.product/tests/TC-069-coverage-primitive-treats-absent-providesevidencef.md`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::ontology::verification_graph::{
    validate_quads, ArtifactRef, StepFields, VerificationGraph, VerificationStep,
};
use decision_cli::core::verify::coverage::feature_resolver::{
    feature_iri_for, graph_iri_for, tc_iri_for,
};
use decision_cli::core::verify::coverage::feature_coverage;
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
            "decision-cli-tc069-{tag}-{}-{}-{}",
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
    body.push_str("title: TC-069 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-069.\n");
    fs::write(dir.join(format!("{feature_id}-fixture.md")), body).expect("write fixture");
}

fn insert_quads(store: &Store, quads: &[oxigraph::model::Quad]) {
    for q in quads {
        store.insert(q.as_ref()).expect("insert");
    }
}

/// Build VG-Y referencing FT-Y but with steps that have NO
/// `dec:providesEvidenceFor` triples.
fn build_vg_y_without_evidence() -> VerificationGraph {
    let step0 = VerificationStep::new(
        "VG-Y",
        0,
        StepFields::ShellCommand {
            command: "echo no-evidence".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    );
    let step1 = VerificationStep::new(
        "VG-Y",
        1,
        StepFields::SparqlAssertion {
            target: ".dec/store".to_string(),
            query: "SELECT ?x WHERE { ?x ?p ?o }".to_string(),
            expect_rows: None,
        },
    );
    // Note: `provides_evidence_for` left as default (empty Vec).
    VerificationGraph::new(
        "VG-Y",
        ArtifactRef(NamedNode::new_unchecked(feature_iri_for("FT-Y"))),
        NamedNode::new_unchecked("https://decision-cli.dev/ns/env/ENV-001-ephemeral-cli"),
        vec![step0, step1],
    )
}

#[test]
fn tc_069_absent_providesevidencefor_yields_zero_coverage() {
    let wd = WorkdirGuard::new("absent-evidence");
    write_feature_fixture(wd.path(), "FT-Y", &["TC-Y1", "TC-Y2"]);

    let store = Store::new().expect("in-memory store");
    let vg = build_vg_y_without_evidence();
    insert_quads(&store, &vg.to_quads(verify_graph_named_graph()));

    let report = feature_coverage(
        "FT-Y",
        Some(vec!["VG-Y".to_string()]),
        &store,
        wd.path(),
    )
    .expect("feature_coverage ok");

    // Acceptance: covered = [], uncovered = FT-Y.tests.
    assert!(report.covered.is_empty(), "no covering hits expected");
    assert_eq!(
        report.uncovered,
        vec![tc_iri_for("TC-Y1"), tc_iri_for("TC-Y2")],
        "every TC must be uncovered"
    );
    // considered = [VG-Y]
    assert_eq!(report.considered, vec![graph_iri_for("VG-Y")]);
}

#[test]
fn tc_069_primitive_does_not_error_on_absent_predicate() {
    let wd = WorkdirGuard::new("no-error");
    write_feature_fixture(wd.path(), "FT-Y", &["TC-Y1", "TC-Y2"]);

    let store = Store::new().expect("in-memory store");
    insert_quads(
        &store,
        &build_vg_y_without_evidence().to_quads(verify_graph_named_graph()),
    );

    // The call must succeed; the empty-coverage shape is the proof
    // the primitive treated absence as "no TC linkage" rather than
    // erroring.
    let report = feature_coverage("FT-Y", None, &store, wd.path())
        .expect("primitive must not error on absent predicate");
    assert!(report.covered.is_empty());
}

#[test]
fn tc_069_graph_remains_shacl_valid_without_predicate() {
    // FT-036's optional-predicate rule: a graph whose steps carry no
    // `dec:providesEvidenceFor` must still pass SHACL validation.
    let vg = build_vg_y_without_evidence();
    let quads = vg.to_quads(verify_graph_named_graph());
    validate_quads(&quads).expect("FT-036 SHACL must still accept absent predicate");
}
