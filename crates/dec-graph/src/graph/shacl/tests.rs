//! Unit tests for the FT-073 validator. See `tests/ft_073_graphwriter_shacl.rs`
//! for the integration test that wires TC-123.

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};

use super::{Validator, RDF_TYPE};
use crate::graph::violation::ViolationKind;
use crate::ontology::{BOUNDARY_ARTIFACT_CLASS, EXTERNAL_ORIGIN_PROP};
use dec_ontology::vocab::{
    IRI_DEC_GRAPH_ORCHESTRATION, IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
    IRI_PROV_WAS_GENERATED_BY,
};

const FEATURE_IRI: &str = "https://decision-cli.dev/ns#Feature";
const FEATURE_INSTANCE: &str = "https://decision-cli.dev/ns/feature/sample";
const SESSION_INSTANCE: &str = "https://decision-cli.dev/ns/session/s1";
const AGENT_INSTANCE: &str = "https://decision-cli.dev/ns/agent/a1";
const BOUNDARY_INSTANCE: &str = "https://decision-cli.dev/ns/feature/boundary";

fn graph() -> GraphName {
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

fn nn(s: &str) -> NamedNode {
    NamedNode::new_unchecked(s)
}

fn rdf_type_quad(subject: &str, class: &str) -> Quad {
    Quad::new(nn(subject), nn(RDF_TYPE), nn(class), graph())
}

fn iri_quad(subject: &str, predicate: &str, object: &str) -> Quad {
    Quad::new(nn(subject), nn(predicate), nn(object), graph())
}

fn lit_quad(subject: &str, predicate: &str, value: &str, datatype: &str) -> Quad {
    Quad::new(
        nn(subject),
        nn(predicate),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype)),
        graph(),
    )
}

fn mechanical(subject: &str) -> Vec<Quad> {
    vec![
        iri_quad(subject, IRI_PROV_WAS_GENERATED_BY, SESSION_INSTANCE),
        iri_quad(
            subject,
            IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
            AGENT_INSTANCE,
        ),
        lit_quad(
            subject,
            IRI_PROV_GENERATED_AT_TIME,
            "2026-05-25T20:00:00Z",
            "http://www.w3.org/2001/XMLSchema#dateTime",
        ),
    ]
}

#[test]
fn validator_loads_with_full_type_table() {
    let v = Validator::load().expect("validator loads");
    let preds = v.motivational_predicates(FEATURE_IRI).expect("Feature");
    assert!(preds.contains(&"https://decision-cli.dev/ns#addresses".to_string()));
    assert!(preds.contains(&"https://decision-cli.dev/ns#decomposesFrom".to_string()));
    assert!(preds.contains(&"https://decision-cli.dev/ns#originatedFrom".to_string()));
    assert!(preds.contains(&"https://decision-cli.dev/ns#respondsTo".to_string()));
}

#[test]
fn rejects_feature_missing_motivational() {
    let v = Validator::load().expect("validator");
    let mut delta = vec![rdf_type_quad(FEATURE_INSTANCE, FEATURE_IRI)];
    delta.extend(mechanical(FEATURE_INSTANCE));
    let report = v.validate(&delta, None);
    assert!(!report.conforms, "delta missing motivational must fail");
    assert!(report.violations.iter().any(|viol| {
        matches!(viol.kind, ViolationKind::MissingMotivational) && viol.artifact == FEATURE_INSTANCE
    }));
    let violation = report
        .violations
        .iter()
        .find(|v| matches!(v.kind, ViolationKind::MissingMotivational))
        .expect("motivational violation");
    assert_eq!(violation.declared_type, FEATURE_IRI);
    assert!(violation
        .accepted_motivational_predicates
        .contains(&"https://decision-cli.dev/ns#addresses".to_string()));
}

#[test]
fn accepts_feature_with_motivational() {
    let v = Validator::load().expect("validator");
    let mut delta = vec![rdf_type_quad(FEATURE_INSTANCE, FEATURE_IRI)];
    delta.extend(mechanical(FEATURE_INSTANCE));
    delta.push(iri_quad(
        FEATURE_INSTANCE,
        "https://decision-cli.dev/ns#addresses",
        "https://decision-cli.dev/ns/feedback/fb1",
    ));
    let report = v.validate(&delta, None);
    assert!(report.conforms, "valid delta should conform: {:?}", report);
}

#[test]
fn accepts_boundary_feature_without_motivational() {
    let v = Validator::load().expect("validator");
    let mut delta = vec![
        rdf_type_quad(BOUNDARY_INSTANCE, FEATURE_IRI),
        rdf_type_quad(BOUNDARY_INSTANCE, BOUNDARY_ARTIFACT_CLASS),
        lit_quad(
            BOUNDARY_INSTANCE,
            EXTERNAL_ORIGIN_PROP,
            "chat://t-2026-05-25",
            "http://www.w3.org/2001/XMLSchema#string",
        ),
    ];
    delta.extend(mechanical(BOUNDARY_INSTANCE));
    let report = v.validate(&delta, None);
    assert!(
        report.conforms,
        "boundary artifact should conform: {:?}",
        report
    );
}

#[test]
fn rejects_missing_mechanical_block() {
    let v = Validator::load().expect("validator");
    let delta = vec![
        rdf_type_quad(FEATURE_INSTANCE, FEATURE_IRI),
        iri_quad(
            FEATURE_INSTANCE,
            "https://decision-cli.dev/ns#addresses",
            "https://decision-cli.dev/ns/feedback/fb1",
        ),
    ];
    let report = v.validate(&delta, None);
    assert!(!report.conforms);
    let mechanical_violations: Vec<_> = report
        .violations
        .iter()
        .filter(|v| matches!(v.kind, ViolationKind::MissingMechanical { .. }))
        .collect();
    assert_eq!(mechanical_violations.len(), 3);
}

#[test]
fn ignores_untyped_subjects() {
    let v = Validator::load().expect("validator");
    let delta = vec![iri_quad(
        "https://decision-cli.dev/ns/raw/random",
        "https://decision-cli.dev/ns#unrelated",
        "https://decision-cli.dev/ns/feedback/fb1",
    )];
    let report = v.validate(&delta, None);
    assert!(report.conforms);
}
