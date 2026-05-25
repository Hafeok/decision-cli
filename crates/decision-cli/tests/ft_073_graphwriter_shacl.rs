//! TC-123 — GraphWriter SHACL enforcement of dual provenance (FT-073 / ADR-041).
//!
//! Exit criterion for FT-073:
//!
//! 1. A `:Feature` write carrying mechanical provenance but no
//!    motivational edge and no BoundaryArtifact class membership is
//!    refused via `validate_and_commit`; the returned error message
//!    starts with `provenance violation` (FT-073 spec
//!    `WriteError::ProvenanceRejected`) and names the artifact, the
//!    declared type, and the slice-1 motivational predicate set for
//!    `:Feature` (from FT-070).
//! 2. A Feedback artifact of class `provenance-violation` is emitted
//!    to the orchestration named graph and is queryable; carries
//!    `dec:sourceSession` pointing back at the producing session per
//!    the FT-029 routing table.
//! 3. The transaction is not committed (a subsequent SELECT confirms the
//!    refused Feature is absent from the store).
//! 4. A conformant `:Feature` write succeeds within the < 50 ms p99
//!    latency budget on the in-memory reference workload.
//! 5. The Python defensive validator (`workers/_shared/shacl`) reaches
//!    the same conformance verdict on the same JSON-N-Quads input. (When
//!    Python tooling is unavailable in the test environment, the agreement
//!    check is skipped with a diagnostic — the Rust-side rejection
//!    remains authoritative.)
//!
//! Runner: `cargo test -p decision-cli --test ft_073_graphwriter_shacl
//! tc_123_graphwriter_rejects_writes_missing_motivational_pr`

use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::graph::{
    validate_and_commit, ProvenanceViolation, ValidateAndCommitOptions, Validator, ViolationKind,
    PROVENANCE_VIOLATION_CLASS,
};
use decision_cli::core::StreamWriter;
use decision_cli::vocab::{
    IRI_DEC_FEEDBACK, IRI_DEC_FEEDBACK_CLASS, IRI_DEC_GRAPH_ORCHESTRATION, IRI_DEC_SOURCE_SESSION,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const FEATURE_CLASS: &str = "https://decision-cli.dev/ns#Feature";
const FEATURE_NEGATIVE: &str = "https://decision-cli.dev/ns/feature/tc123-bad";
const FEATURE_POSITIVE: &str = "https://decision-cli.dev/ns/feature/tc123-good";
const FEEDBACK_VIOLATION_IRI: &str = "https://decision-cli.dev/ns/feedback/tc123-violation";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/tc123-s1";
const AGENT_IRI: &str = "https://decision-cli.dev/ns/agent/tc123-a1";
const STREAM_IRI: &str = "https://decision-cli.dev/ns/stream/tc123-development";
const TIMESTAMP: &str = "2026-05-25T20:30:00Z";

// ---------------------------------------------------------------------------
// TC-123 — the headline test the TC frontmatter points at.
// ---------------------------------------------------------------------------

#[test]
fn tc_123_graphwriter_rejects_writes_missing_motivational_pr() {
    let (store, writer) = open_writer();
    let validator = Validator::load().expect("validator loads");

    // ----- (1) Negative case: Feature without motivational edge ----------
    let bad_mutation = build_feature_mutation(FEATURE_NEGATIVE, /*with_motivational=*/ false);
    let opts = ValidateAndCommitOptions::pass_through(
        NamedNode::new(FEEDBACK_VIOLATION_IRI).expect("feedback iri"),
        NamedNode::new(SESSION_IRI).expect("session iri"),
    );
    let outcome = validate_and_commit(&writer, &validator, bad_mutation, &opts);
    let err = outcome.expect_err("FT-073 must refuse motivational-less writes");
    let rendered = err.to_string();
    assert!(
        rendered.starts_with("provenance violation"),
        "error prefix must be `provenance violation`, got: {rendered}"
    );
    assert!(
        rendered.contains(FEATURE_NEGATIVE),
        "violation must name the artifact IRI: {rendered}"
    );
    assert!(
        rendered.contains(FEATURE_CLASS),
        "violation must name the declared type: {rendered}"
    );
    for pred in [
        "https://decision-cli.dev/ns#addresses",
        "https://decision-cli.dev/ns#decomposesFrom",
        "https://decision-cli.dev/ns#originatedFrom",
        "https://decision-cli.dev/ns#respondsTo",
    ] {
        assert!(
            rendered.contains(pred),
            "violation must name slice-1 motivational predicate {pred}: {rendered}"
        );
    }

    // ----- (2) The refused Feature is NOT in the store ------------------
    assert!(
        !subject_present(&store, FEATURE_NEGATIVE),
        "refused Feature must not be committed"
    );

    // ----- (3) The Feedback artifact IS in the store --------------------
    assert!(
        subject_present(&store, FEEDBACK_VIOLATION_IRI),
        "violation Feedback must be emitted to the orchestration store"
    );
    let feedback_class = literal_value_of(
        &store,
        FEEDBACK_VIOLATION_IRI,
        IRI_DEC_FEEDBACK_CLASS,
    );
    assert_eq!(
        feedback_class.as_deref(),
        Some(PROVENANCE_VIOLATION_CLASS),
        "feedback must declare class `provenance-violation`"
    );
    let source_session = iri_value_of(&store, FEEDBACK_VIOLATION_IRI, IRI_DEC_SOURCE_SESSION);
    assert_eq!(
        source_session.as_deref(),
        Some(SESSION_IRI),
        "feedback must route back to the producing session"
    );
    assert!(
        feedback_is_typed_as_feedback(&store, FEEDBACK_VIOLATION_IRI),
        "emitted Feedback must carry rdf:type dec:Feedback"
    );

    // ----- (4) Positive case: a conformant Feature commits in < 50ms ----
    let good_mutation = build_feature_mutation(FEATURE_POSITIVE, /*with_motivational=*/ true);
    let opts2 = ValidateAndCommitOptions::pass_through(
        NamedNode::new("https://decision-cli.dev/ns/feedback/tc123-violation-good")
            .expect("feedback iri"),
        NamedNode::new(SESSION_IRI).expect("session iri"),
    );
    let started = Instant::now();
    let receipt = validate_and_commit(&writer, &validator, good_mutation, &opts2)
        .expect("conformant Feature must commit");
    let elapsed = started.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "FT-073 §Invariants: validation+commit must fit < 50 ms p99 (got {:?})",
        elapsed
    );
    let _ = receipt; // CommitResult ignored — the SELECT below is the canonical assertion.
    assert!(
        subject_present(&store, FEATURE_POSITIVE),
        "conformant Feature must be present after commit"
    );

    // ----- (5) Dual-validator agreement (best-effort) -------------------
    dual_validator_agreement_check();
}

// ---------------------------------------------------------------------------
// Helpers — store / mutation construction.
// ---------------------------------------------------------------------------

fn open_writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, writer)
}

fn build_feature_mutation(iri: &str, with_motivational: bool) -> Mutation {
    let mut quads: Vec<Quad> = Vec::new();
    quads.push(typed_quad(iri, RDF_TYPE, FEATURE_CLASS));
    // Caller-supplied mechanical block — FT-073 §Behaviour step 1 also
    // covers materialisation, but pass-through mode lets the test
    // pinpoint motivational rejection without mocking session records.
    quads.extend(mechanical_block_for(iri));
    if with_motivational {
        quads.push(typed_quad(
            iri,
            "https://decision-cli.dev/ns#addresses",
            "https://decision-cli.dev/ns/feedback/tc123-source",
        ));
    }
    Mutation::insert(quads)
}

fn mechanical_block_for(subject: &str) -> Vec<Quad> {
    let g = orchestration_graph();
    vec![
        Quad::new(
            NamedNode::new_unchecked(subject),
            NamedNode::new_unchecked("http://www.w3.org/ns/prov#wasGeneratedBy"),
            NamedNode::new_unchecked(SESSION_IRI),
            g.clone(),
        ),
        Quad::new(
            NamedNode::new_unchecked(subject),
            NamedNode::new_unchecked("http://www.w3.org/ns/prov#wasAttributedTo"),
            NamedNode::new_unchecked(AGENT_IRI),
            g.clone(),
        ),
        Quad::new(
            NamedNode::new_unchecked(subject),
            NamedNode::new_unchecked("http://www.w3.org/ns/prov#generatedAtTime"),
            Literal::new_typed_literal(
                TIMESTAMP,
                NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
            ),
            g,
        ),
    ]
}

fn typed_quad(subject: &str, predicate: &str, object: &str) -> Quad {
    Quad::new(
        NamedNode::new_unchecked(subject),
        NamedNode::new_unchecked(predicate),
        NamedNode::new_unchecked(object),
        orchestration_graph(),
    )
}

fn orchestration_graph() -> GraphName {
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

// ---------------------------------------------------------------------------
// Helpers — SPARQL probes.
// ---------------------------------------------------------------------------

fn subject_present(store: &Store, subject: &str) -> bool {
    let q = format!(
        "ASK {{ {{ <{s}> ?p ?o }} UNION {{ GRAPH ?g {{ <{s}> ?p ?o }} }} }}",
        s = subject,
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

fn literal_value_of(store: &Store, subject: &str, predicate: &str) -> Option<String> {
    let q = format!(
        "SELECT ?v WHERE {{ {{ <{s}> <{p}> ?v }} UNION {{ GRAPH ?g {{ <{s}> <{p}> ?v }} }} }}",
        s = subject,
        p = predicate,
    );
    match store.query(q.as_str()).ok()? {
        QueryResults::Solutions(sols) => {
            for sol in sols.flatten() {
                if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("v") {
                    return Some(lit.value().to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn iri_value_of(store: &Store, subject: &str, predicate: &str) -> Option<String> {
    let q = format!(
        "SELECT ?v WHERE {{ {{ <{s}> <{p}> ?v }} UNION {{ GRAPH ?g {{ <{s}> <{p}> ?v }} }} }}",
        s = subject,
        p = predicate,
    );
    match store.query(q.as_str()).ok()? {
        QueryResults::Solutions(sols) => {
            for sol in sols.flatten() {
                if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("v") {
                    return Some(n.as_str().to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn feedback_is_typed_as_feedback(store: &Store, subject: &str) -> bool {
    let q = format!(
        "ASK {{ {{ <{s}> a <{c}> }} UNION {{ GRAPH ?g {{ <{s}> a <{c}> }} }} }}",
        s = subject,
        c = IRI_DEC_FEEDBACK,
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

// ---------------------------------------------------------------------------
// Dual-validator agreement (FT-073 §7).
// ---------------------------------------------------------------------------

/// Best-effort agreement check: feed the same N-Quads payload to the
/// Python defensive validator (`workers/_shared/shacl.py`). If pyoxigraph
/// or the venv is not available, log and skip — the Rust-side rejection
/// remains the authoritative check the TC asserts. Failure to run does
/// not fail the test; *disagreement* does.
fn dual_validator_agreement_check() {
    let script = workspace_relative("workers/_shared/src/_shared/shacl_check.py");
    if !script.exists() {
        eprintln!(
            "[FT-073] dual-validator check skipped — script not present at {}",
            script.display()
        );
        return;
    }
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());
    let probe = Command::new(&python).arg("--version").output();
    if probe.is_err() {
        eprintln!("[FT-073] dual-validator check skipped — `{python}` unavailable");
        return;
    }
    // Construct a representative negative case (Feature missing motivational)
    // serialised as N-Quads on stdin. The Python validator should report
    // non-conformance.
    let nquads = sample_negative_nquads();
    let mut child = match Command::new(&python)
        .arg(script.as_path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[FT-073] dual-validator check skipped — spawn failed: {err}");
            return;
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(nquads.as_bytes());
    }
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(err) => {
            eprintln!("[FT-073] dual-validator check skipped — wait failed: {err}");
            return;
        }
    };
    // Convention: exit 0 = conforms; exit 1 = violation. The script may
    // also return exit 2 when its deps are missing — treat as skip.
    let status = out.status.code();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if status == Some(2) {
        eprintln!(
            "[FT-073] dual-validator check skipped — script reports deps missing\n{stderr}"
        );
        return;
    }
    assert_eq!(
        status,
        Some(1),
        "dual-validator must reject the same delta the Rust side rejects.\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stdout.contains("MissingMotivational") || stdout.contains("motivational"),
        "Python validator output must name the motivational failure: {stdout}",
    );
}

fn sample_negative_nquads() -> String {
    let g = IRI_DEC_GRAPH_ORCHESTRATION;
    [
        format!("<{FEATURE_NEGATIVE}> <{RDF_TYPE}> <{FEATURE_CLASS}> <{g}> ."),
        format!(
            "<{FEATURE_NEGATIVE}> <http://www.w3.org/ns/prov#wasGeneratedBy> <{SESSION_IRI}> <{g}> ."
        ),
        format!(
            "<{FEATURE_NEGATIVE}> <http://www.w3.org/ns/prov#wasAttributedTo> <{AGENT_IRI}> <{g}> ."
        ),
        format!(
            "<{FEATURE_NEGATIVE}> <http://www.w3.org/ns/prov#generatedAtTime> \"{TIMESTAMP}\"^^<http://www.w3.org/2001/XMLSchema#dateTime> <{g}> ."
        ),
    ]
    .join("\n")
}

fn workspace_relative(suffix: &str) -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR points at crates/decision-cli; the workspace root
    // is two parents up.
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join(suffix))
        .unwrap_or_else(|| std::path::PathBuf::from(suffix))
}

// ---------------------------------------------------------------------------
// Extra unit-level assertions — keep the slice-1 violator/validator API
// stable so downstream features (FT-074 migration tooling) read the same
// shape.
// ---------------------------------------------------------------------------

#[test]
fn provenance_violation_struct_round_trips_through_serde() {
    let v = ProvenanceViolation::new(
        &NamedNode::new(FEATURE_NEGATIVE).unwrap(),
        FEATURE_CLASS,
        ViolationKind::MissingMotivational,
        vec!["https://decision-cli.dev/ns#addresses".into()],
    );
    let json = serde_json::to_string(&v).expect("ser");
    let back: ProvenanceViolation = serde_json::from_str(&json).expect("de");
    assert_eq!(back, v);
}
