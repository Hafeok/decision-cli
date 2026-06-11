//! Unit tests for FT-071 / ADR-040 BoundaryArtifact validators.

use oxrdf::{GraphName, Literal, NamedNode, Quad};

use crate::ontology::{EXTERNAL_ORIGIN_PROP, IS_MIGRATION_BACKFILL_PROP};
use crate::vocab::IRI_DEC_GRAPH_ORCHESTRATION;

use super::{validate_boundary_artifact, validate_migration_backfill};

fn artifact() -> NamedNode {
    NamedNode::new_unchecked("https://decision-cli.dev/ns/artifact/ba1")
}

fn graph() -> GraphName {
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

fn external_origin_quad(subject: &NamedNode, value: &str) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(EXTERNAL_ORIGIN_PROP),
        Literal::new_simple_literal(value),
        graph(),
    )
}

fn is_migration_backfill_quad(subject: &NamedNode, value: bool) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(IS_MIGRATION_BACKFILL_PROP),
        Literal::new_typed_literal(
            if value { "true" } else { "false" },
            NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean"),
        ),
        graph(),
    )
}

#[test]
fn well_formed_external_origin_validates() {
    let a = artifact();
    let quads = vec![external_origin_quad(&a, "chat-transcript:abc123")];
    validate_boundary_artifact(&quads, &a)
        .expect("non-empty xsd:string external_origin must validate");
}

#[test]
fn missing_external_origin_is_rejected() {
    let a = artifact();
    let err = validate_boundary_artifact(&[], &a)
        .expect_err("missing dec:external_origin must fail :BoundaryArtifactShape");
    assert!(err.report.contains("external_origin"), "{}", err.report);
    assert!(err.report.contains("missing required"), "{}", err.report);
}

#[test]
fn empty_external_origin_is_rejected() {
    let a = artifact();
    let quads = vec![external_origin_quad(&a, "")];
    let err = validate_boundary_artifact(&quads, &a)
        .expect_err("empty dec:external_origin must fail sh:minLength");
    assert!(err.report.contains("non-empty"), "{}", err.report);
}

#[test]
fn duplicate_external_origin_is_rejected() {
    let a = artifact();
    let quads = vec![
        external_origin_quad(&a, "first"),
        external_origin_quad(&a, "second"),
    ];
    let err = validate_boundary_artifact(&quads, &a)
        .expect_err("two dec:external_origin literals must fail sh:maxCount");
    assert!(
        err.report.contains("expected exactly one"),
        "{}",
        err.report
    );
}

#[test]
fn well_formed_is_migration_backfill_validates() {
    let a = artifact();
    let quads = vec![is_migration_backfill_quad(&a, true)];
    validate_migration_backfill(&quads, &a).expect("dec:isMigrationBackfill true must validate");
}

#[test]
fn missing_is_migration_backfill_is_rejected() {
    let a = artifact();
    let err = validate_migration_backfill(&[], &a)
        .expect_err("missing dec:isMigrationBackfill must fail :MigrationBackfillShape");
    assert!(err.report.contains("isMigrationBackfill"), "{}", err.report);
    assert!(err.report.contains("missing required"), "{}", err.report);
}

#[test]
fn is_migration_backfill_false_is_rejected() {
    let a = artifact();
    let quads = vec![is_migration_backfill_quad(&a, false)];
    let err = validate_migration_backfill(&quads, &a)
        .expect_err("dec:isMigrationBackfill false must fail sh:hasValue true");
    assert!(err.report.contains("true"), "{}", err.report);
}
