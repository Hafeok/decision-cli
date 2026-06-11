//! TC-119 — universal mechanical-provenance SHACL fragment (FT-069 / ADR-038).
//!
//! Exit criterion for FT-069:
//!
//! 1. `dec init` succeeds on a fresh store with FT-069's
//!    `mechanical-provenance.ttl` loaded — exercised by loading
//!    `OntologyHandle` and asserting both `:MechanicalProvenanceShape`
//!    and `:SessionProvenanceShape` are reachable in the shapes graph.
//! 2. A fixture artifact's mechanical block materialised from a
//!    `SessionAttribution` validates against `:MechanicalProvenanceShape`
//!    (SHACL `conforms = true` in W3C parlance).
//! 3. A negative test strips `prov:wasGeneratedBy` from the same fixture
//!    and asserts the validator reports a violation naming that property
//!    path.
//! 4. The Rust IRI constants exposed in `core/ontology` match the IRIs
//!    written in the TTL byte-for-byte (no drift between the
//!    machine-loaded shape and the documented constant).

use oxigraph::model::{NamedNode, NamedNodeRef, Subject, Term};

use decision_cli::core::ontology::mechanical_provenance::{
    materialise_quads, validate_quads, validate_subject, SessionAttribution,
};
use decision_cli::core::ontology::{
    OntologyHandle, MECHANICAL_PROVENANCE_SHAPE, MECHANICAL_PROVENANCE_SHAPES_TTL,
    SESSION_PROVENANCE_SHAPE, SHAPES_GRAPH_IRI,
};
use decision_cli::vocab::{
    IRI_DEC_AGENT, IRI_DEC_ARTIFACT, IRI_DEC_GRAPH_ORCHESTRATION, IRI_PROV_GENERATED_AT_TIME,
    IRI_PROV_WAS_ASSOCIATED_WITH, IRI_PROV_WAS_GENERATED_BY, IRI_XSD_DATE_TIME,
};

const TS: &str = "2026-05-25T18:00:00Z";

fn artifact() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/artifact/tc119-x")
}

fn session() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/session/tc119-s1")
}

fn agent() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/agent/tc119-a1")
}

fn graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION)
}

// --- The five-line headline assertion the TC exists for --------------------

/// TC-119: the universal mechanical-provenance SHACL fragment loads
/// cleanly and validates an artifact whose mechanical block was
/// materialised by [`materialise_quads`] from a session record (positive
/// case + negative case in a single test method, matching the
/// runner-args recorded in the TC frontmatter).
#[test]
fn loads_and_validates_session_record() {
    // (1) dec init's ontology bootstrap succeeds with FT-069's shape
    // file in the shapes graph. The handle's invariant checks
    // (`invariant_mechanical_provenance_shapes_present`) panic at load
    // time if either NodeShape is missing.
    let handle = OntologyHandle::load().expect("ontology + FT-069 shapes load");
    assert_shape_present(handle, MECHANICAL_PROVENANCE_SHAPE);
    assert_shape_present(handle, SESSION_PROVENANCE_SHAPE);

    // (2) GraphWriter materialises a mechanical block from the
    // SessionAttribution. We assert the three required triples appear,
    // and SHACL validation returns conforms = true.
    let a = artifact();
    let attribution = SessionAttribution::new(session(), vec![agent()], TS);
    let quads = materialise_quads(&a, &attribution, graph());

    let preds: Vec<&str> = quads.iter().map(|q| q.predicate.as_str()).collect();
    assert!(
        preds.contains(&"http://www.w3.org/ns/prov#wasGeneratedBy"),
        "expected prov:wasGeneratedBy in materialised mechanical block: {preds:?}"
    );
    assert!(
        preds.contains(&"http://www.w3.org/ns/prov#wasAttributedTo"),
        "expected prov:wasAttributedTo in materialised mechanical block: {preds:?}"
    );
    assert!(
        preds.contains(&"http://www.w3.org/ns/prov#generatedAtTime"),
        "expected prov:generatedAtTime in materialised mechanical block: {preds:?}"
    );

    validate_quads(&quads, std::slice::from_ref(&a))
        .expect("materialised mechanical block must satisfy :MechanicalProvenanceShape");
    assert!(
        validate_subject(&quads, &a).is_empty(),
        "no per-subject violations expected for a well-formed mechanical block"
    );

    // (3) Negative — strip prov:wasGeneratedBy, expect a structured
    // violation naming the property path.
    let mut stripped = quads.clone();
    stripped.retain(|q| q.predicate.as_str() != "http://www.w3.org/ns/prov#wasGeneratedBy");
    let err = validate_quads(&stripped, std::slice::from_ref(&a))
        .expect_err("stripped wasGeneratedBy must produce a SHACL violation");
    assert!(
        err.report
            .contains("http://www.w3.org/ns/prov#wasGeneratedBy"),
        "violation report must name the property path; got: {}",
        err.report
    );
    let has_path_violation = err
        .violations
        .iter()
        .any(|v| v.path == "http://www.w3.org/ns/prov#wasGeneratedBy");
    assert!(
        has_path_violation,
        "violation list must include the wasGeneratedBy path; got: {:?}",
        err.violations
    );

    // (4) IRI byte-for-byte parity between the Rust constants and the
    // shape file. Reading the shape TTL as a string and grepping the
    // constants is the simplest possible drift detector — any rename
    // on either side fails this test loudly.
    let ttl = MECHANICAL_PROVENANCE_SHAPES_TTL;
    let expected = [
        MECHANICAL_PROVENANCE_SHAPE,
        SESSION_PROVENANCE_SHAPE,
        IRI_PROV_WAS_GENERATED_BY,
        IRI_PROV_GENERATED_AT_TIME,
        IRI_PROV_WAS_ASSOCIATED_WITH,
        IRI_DEC_AGENT,
        IRI_DEC_ARTIFACT,
        IRI_XSD_DATE_TIME,
    ];
    for iri in expected {
        let local_or_prefixed = ttl.contains(iri) || contains_prefixed_form(ttl, iri);
        assert!(
            local_or_prefixed,
            "expected IRI {iri:?} (or its prefixed shorthand) to appear in the embedded shape TTL"
        );
    }
}

// --- Helpers ----------------------------------------------------------------

fn assert_shape_present(handle: &OntologyHandle, shape_iri: &str) {
    let target = NamedNode::new_unchecked(shape_iri);
    let mut found_shape = false;
    for quad in handle.shapes_graph() {
        if quad.predicate.as_str() != "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" {
            continue;
        }
        let Subject::NamedNode(s) = &quad.subject else {
            continue;
        };
        if s != &target {
            continue;
        }
        if let Term::NamedNode(cls) = &quad.object {
            if cls.as_str() == "http://www.w3.org/ns/shacl#NodeShape" {
                found_shape = true;
                break;
            }
        }
    }
    assert!(
        found_shape,
        "shapes graph at <{SHAPES_GRAPH_IRI}> must declare <{shape_iri}> as sh:NodeShape"
    );
}

/// True if `ttl` contains the prefixed shorthand of `full_iri` for the
/// `dec:` / `prov:` / `xsd:` / `sh:` namespaces. We accept either the
/// full IRI or the shorthand to keep this drift check resilient against
/// stylistic Turtle authoring decisions.
fn contains_prefixed_form(ttl: &str, full_iri: &str) -> bool {
    for (prefix, namespace) in [
        ("dec:", "https://decision-cli.dev/ns#"),
        ("prov:", "http://www.w3.org/ns/prov#"),
        ("xsd:", "http://www.w3.org/2001/XMLSchema#"),
        ("sh:", "http://www.w3.org/ns/shacl#"),
    ] {
        if let Some(local) = full_iri.strip_prefix(namespace) {
            let prefixed = format!("{prefix}{local}");
            if ttl.contains(&prefixed) {
                return true;
            }
        }
    }
    false
}
