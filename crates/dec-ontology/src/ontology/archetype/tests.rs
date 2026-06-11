//! FT-147 round-trip and negative-SHACL tests, per the spec's four cases.

use oxrdf::{NamedNode, NamedNodeRef};

use crate::ontology::provenance::{MotivationalEdge, Provenance};
use crate::vocab::{IRI_DEC_APPLICATION_CONTRACT, IRI_DEC_ARCHETYPE_STATUS};

use super::parser::quads_to_archetype;
use super::shacl::{validate_quads, E102_CODE};
use super::types::{archetype_iri, Archetype, ArchetypeEvidence, ArchetypeStatus, Variance};

const TEST_GRAPH: &str = "https://decision-cli.dev/ns/orchestration";

fn graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(TEST_GRAPH)
}

fn n(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

/// An archetype with three seam audits — the spec's positive fixture.
fn fixture() -> Archetype {
    Archetype {
        id: archetype_iri("self-service-portal"),
        title: "Self-Service Portal".to_string(),
        status: ArchetypeStatus::Candidate,
        application_contract: n("https://decision-cli.dev/ns/contract/app/ssp"),
        infrastructure_contract_template: n("https://decision-cli.dev/ns/contract/infra/ssp"),
        infrastructure_contract_instances: vec![n(
            "https://decision-cli.dev/ns/contract/infra/ssp/instance-1",
        )],
        application_task_types: vec![
            n("https://decision-cli.dev/ns/task-type/add-crud-entity"),
            n("https://decision-cli.dev/ns/task-type/add-list-view"),
        ],
        infrastructure_task_types: vec![n(
            "https://decision-cli.dev/ns/task-type/provision-postgres",
        )],
        archetype_audits: vec![n("https://decision-cli.dev/ns/audit/archetype/ssp-1")],
        seam_audits: vec![
            n("https://decision-cli.dev/ns/audit/seam/ssp-auth-data"),
            n("https://decision-cli.dev/ns/audit/seam/ssp-ui-api"),
            n("https://decision-cli.dev/ns/audit/seam/ssp-api-store"),
        ],
        evidence: ArchetypeEvidence {
            archetype_layer_estimate: 0.65,
            instance_variance: Variance::Medium,
            application_contract_held_invariant: true,
            coverage_note: "Estimate covers CRUD + auth flows across 3 instances.".to_string(),
        },
        provenance: Provenance {
            was_generated_by: n("https://decision-cli.dev/ns/session/test-session-1"),
            was_attributed_to: n("https://decision-cli.dev/ns/agent/archetype-pattern-extractor"),
            generated_at_time: "2026-06-11T12:00:00Z".to_string(),
            motivational: vec![MotivationalEdge {
                predicate: n("https://decision-cli.dev/ns#respondsTo"),
                target: n("https://decision-cli.dev/ns/feature/FT-147"),
            }],
        },
    }
}

/// Spec case 1 — positive round-trip: emit, validate, parse, assert
/// structural equality.
#[test]
fn round_trip_with_three_seam_audits() {
    let original = fixture();
    let quads = original.to_quads(graph());
    validate_quads(&quads).expect("fixture passes SHACL");
    let parsed = quads_to_archetype(&quads).expect("fixture parses back");
    assert_eq!(original, parsed);
}

/// Spec case 2 — zero seam audits must be rejected with E102 (ADR-084 §1).
#[test]
fn empty_seam_audits_fails_shacl_with_e102() {
    let mut archetype = fixture();
    archetype.seam_audits.clear();
    let quads = archetype.to_quads(graph());
    let err = validate_quads(&quads).expect_err("empty seam-audit set must fail");
    assert!(err.report.contains(E102_CODE), "{}", err.report);
}

/// Spec case 3 — an invalid status literal must be rejected.
#[test]
fn invalid_status_fails_shacl() {
    let mut quads = fixture().to_quads(graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_ARCHETYPE_STATUS {
            q.object = oxrdf::Literal::new_simple_literal("experimental").into();
        }
    }
    let err = validate_quads(&quads).expect_err("unknown status must fail");
    assert!(err.report.contains("dec:status"), "{}", err.report);
}

/// Spec case 4 — a missing application_contract link must be rejected.
#[test]
fn missing_application_contract_fails_shacl() {
    let mut quads = fixture().to_quads(graph());
    quads.retain(|q| q.predicate.as_str() != IRI_DEC_APPLICATION_CONTRACT);
    let err = validate_quads(&quads).expect_err("missing application contract must fail");
    assert!(
        err.report.contains("dec:applicationContract"),
        "{}",
        err.report
    );
}

/// The parser refuses an archetype whose provenance block is absent —
/// dual provenance is mandatory per FT-072/FT-073.
#[test]
fn missing_provenance_fails_parse() {
    let mut quads = fixture().to_quads(graph());
    quads.retain(|q| {
        !q.predicate
            .as_str()
            .starts_with("http://www.w3.org/ns/prov#")
    });
    let err = quads_to_archetype(&quads).expect_err("missing provenance must fail parse");
    assert!(err.to_string().contains("provenance"), "{err}");
}
