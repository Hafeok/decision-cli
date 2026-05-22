//! TC-029 — VerificationVerdict conforms to ADR-018 SHACL shape.
//!
//! Validates FT-020 / ADR-018: every persisted `dec:VerificationVerdict`
//! passes the SHACL shape declared in the embedded ontology bundle, and
//! the [`StreamWriter`] write-side validation refuses malformed verdicts
//! before they touch the orchestration store.

use std::sync::Arc;

use decision_cli::core::ontology::verdict::{validate_quads, Verdict, VerdictArtifact};
use decision_cli::vocab::orchestration_graph;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad, Subject};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-stream";
const SESSION_IRI: &str = "https://decision-cli.dev/ns/session/interp-1";
const VERDICT_IRI: &str = "urn:dec:verdict:tc-029:1";

const DEC_VERIFICATION_VERDICT: &str = "https://decision-cli.dev/ns#VerificationVerdict";
const DEC_VERDICT: &str = "https://decision-cli.dev/ns#verdict";
const DEC_RATIONALE: &str = "https://decision-cli.dev/ns#rationale";
const DEC_IN_STREAM: &str = "https://decision-cli.dev/ns#inStream";
const PROV_USED: &str = "http://www.w3.org/ns/prov#used";
const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";

fn approved_artifact() -> VerdictArtifact {
    VerdictArtifact {
        iri: NamedNode::new(VERDICT_IRI).expect("verdict iri"),
        verdict: Verdict::Approved,
        rationale: "Approved — produced CodeChange satisfies all listed TCs.".to_string(),
        violates: Vec::new(),
        amendment_guidance: None,
        generated_by: NamedNode::new(SESSION_IRI).expect("session iri"),
        used: vec![NamedNode::new("urn:dec:code-change:1").expect("cc iri")],
        in_stream: NamedNode::new(STREAM_IRI).expect("stream iri"),
    }
}

#[test]
fn approved_verdict_serialises_and_passes_shacl() {
    let a = approved_artifact();
    let quads = a.to_quads(orchestration_graph());

    // Every required property is present in the produced quad set.
    assert!(predicate_present(&quads, DEC_VERDICT));
    assert!(predicate_present(&quads, DEC_RATIONALE));
    assert!(predicate_present(&quads, DEC_IN_STREAM));
    assert!(predicate_present(&quads, PROV_USED));
    assert!(predicate_present(&quads, PROV_WAS_GENERATED_BY));

    validate_quads(&quads).expect("ADR-018 SHACL shape accepts a well-formed approved verdict");
}

#[test]
fn missing_required_property_fails_shacl() {
    let mut quads = approved_artifact().to_quads(orchestration_graph());
    // Remove the verdict literal — must violate sh:minCount 1.
    quads.retain(|q| q.predicate.as_str() != DEC_VERDICT);
    let err = validate_quads(&quads).expect_err("SHACL must reject verdict missing dec:verdict");
    assert!(
        err.report.contains("dec:verdict"),
        "report should mention dec:verdict; got:\n{}",
        err.report
    );
}

#[test]
fn stream_writer_refuses_malformed_verdict_pre_commit() {
    // ADR-018 §"Write-side validation": the StreamWriter must reject a
    // malformed verdict before persisting any quads.
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let g: GraphName = orchestration_graph().into_owned().into();
    let v = NamedNode::new(VERDICT_IRI).expect("verdict iri");
    let cls = NamedNodeRef::new(DEC_VERIFICATION_VERDICT).expect("class iri");
    let type_pred =
        NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").expect("rdf:type");

    // Build a deliberately incomplete verdict: type only — no rationale,
    // no prov:wasGeneratedBy, no prov:used.
    let mut mutation = Mutation::default();
    mutation
        .inserts
        .push(Quad::new(v.clone(), type_pred, cls.into_owned(), g));

    let err = writer
        .commit(mutation)
        .expect_err("StreamWriter must refuse a verdict missing required fields");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("SHACL violation"),
        "error should be tagged as a SHACL violation; got: {msg}"
    );

    // The store must NOT contain the verdict node after the failed commit.
    let exists = store
        .quads_for_pattern(Some(Subject::NamedNode(v).as_ref()), None, None, None)
        .next()
        .is_some();
    assert!(
        !exists,
        "verdict quads must not be persisted when SHACL validation fails"
    );
}

#[test]
fn stream_writer_accepts_well_formed_verdict() {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    let a = approved_artifact();
    let quads = a.to_quads(orchestration_graph());
    let mutation = Mutation::insert(quads.iter().cloned());
    writer
        .commit(mutation)
        .expect("a well-formed VerificationVerdict commits cleanly");

    // The verdict node is now in the store.
    let v = NamedNode::new(VERDICT_IRI).expect("verdict iri");
    let exists = store
        .quads_for_pattern(Some(Subject::NamedNode(v).as_ref()), None, None, None)
        .next()
        .is_some();
    assert!(
        exists,
        "verdict must be persisted after a successful commit"
    );
}

#[test]
fn embedded_shapes_declare_verification_verdict_shape() {
    use decision_cli::OntologyHandle;

    let h = OntologyHandle::load().expect("load ontology");
    // The embedded shapes graph must declare a shape targeting
    // dec:VerificationVerdict (ADR-018 §SHACL shape).
    let shape_quads: Vec<_> = h.shapes_graph().collect();
    let target =
        NamedNode::new("https://decision-cli.dev/ns#VerificationVerdict").expect("class iri");
    let mut has_shape = false;
    for q in &shape_quads {
        if q.predicate.as_str() == "http://www.w3.org/ns/shacl#targetClass" {
            if let oxigraph::model::Term::NamedNode(n) = &q.object {
                if n == &target {
                    has_shape = true;
                    break;
                }
            }
        }
    }
    assert!(
        has_shape,
        "shapes.ttl must declare a sh:NodeShape with sh:targetClass dec:VerificationVerdict"
    );
}

fn predicate_present(quads: &[Quad], predicate: &str) -> bool {
    quads.iter().any(|q| q.predicate.as_str() == predicate)
}
