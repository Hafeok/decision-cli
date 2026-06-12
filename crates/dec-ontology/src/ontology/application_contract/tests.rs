//! FT-148 round-trip and negative-SHACL tests, per the spec's cases.

use std::path::PathBuf;

use oxrdf::{NamedNode, NamedNodeRef};

use crate::ontology::provenance::Provenance;
use crate::vocab::{IRI_DEC_CONVENTION_BODY_PATH, IRI_DEC_LANGUAGE_RUNTIME};

use super::parser::quads_to_application_contract;
use super::shacl::validate_quads;
use super::types::{application_contract_iri, ApplicationContract, Convention};

fn graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked("https://decision-cli.dev/ns/orchestration")
}

fn n(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

fn convention(name: &str, checkable: bool) -> Convention {
    Convention {
        id: n(&format!(
            "https://decision-cli.dev/ns/contract/app/ssp/convention/{name}"
        )),
        name: name.to_string(),
        body_path: PathBuf::from(format!(
            "forge/archetypes/ssp/application/conventions/{name}.md"
        )),
        audit_id: Some(n(&format!(
            "https://decision-cli.dev/ns/audit/archetype/ssp-{name}"
        ))),
        checkable,
    }
}

/// Spec positive fixture: six conventions + three cross-cutting entries.
fn fixture() -> ApplicationContract {
    ApplicationContract {
        id: application_contract_iri("ssp"),
        archetype: n("https://decision-cli.dev/ns/archetype/self-service-portal"),
        language_runtime: convention("language-runtime", true),
        layering_rule: convention("clean-architecture", true),
        feature_organisation: convention("vertical-slices", true),
        persistence_model: convention("persistence", true),
        endpoint_convention: convention("endpoint-quad", true),
        cross_cutting: vec![
            convention("auth", true),
            convention("validation", true),
            convention("error-handling", false),
        ],
        provenance: Provenance {
            was_generated_by: n("https://decision-cli.dev/ns/session/test-session-148"),
            was_attributed_to: n("https://decision-cli.dev/ns/agent/archetype-pattern-extractor"),
            generated_at_time: "2026-06-12T12:00:00Z".to_string(),
            motivational: vec![],
        },
    }
}

/// Spec case 1 — positive round-trip with all six conventions plus
/// cross-cutting entries: emit, validate, parse, assert equality.
#[test]
fn round_trip_full_contract() {
    let original = fixture();
    let quads = original.to_quads(graph());
    validate_quads(&quads).expect("fixture passes SHACL");
    let parsed = quads_to_application_contract(&quads).expect("fixture parses back");
    assert_eq!(original, parsed);
}

/// Spec case 2 — a missing required Convention link is rejected.
#[test]
fn missing_required_convention_fails_shacl() {
    let mut quads = fixture().to_quads(graph());
    quads.retain(|q| q.predicate.as_str() != IRI_DEC_LANGUAGE_RUNTIME);
    let err = validate_quads(&quads).expect_err("missing languageRuntime must fail");
    assert!(err.report.contains("dec:languageRuntime"), "{}", err.report);
}

/// Spec case 3 — a Convention with an empty body_path is rejected.
#[test]
fn empty_body_path_fails_shacl() {
    let mut quads = fixture().to_quads(graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_CONVENTION_BODY_PATH {
            q.object = oxrdf::Literal::new_simple_literal("").into();
        }
    }
    let err = validate_quads(&quads).expect_err("empty body_path must fail");
    assert!(
        err.report.contains("dec:conventionBodyPath"),
        "{}",
        err.report
    );
}

/// Spec case 4 (placeholder per spec) — `checkable: false` round-trips
/// faithfully; the not-safely-dispatchable propagation itself ships in
/// FT-150/FT-153 and is asserted there.
#[test]
fn uncheckable_convention_round_trips() {
    let contract = fixture();
    let quads = contract.to_quads(graph());
    validate_quads(&quads).expect("checkable:false is valid (consequence is downstream)");
    let parsed = quads_to_application_contract(&quads).expect("parses");
    assert!(!parsed.cross_cutting[2].checkable);
    assert_eq!(parsed.conventions().len(), 8);
}
