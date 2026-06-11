//! FT-147 store-side tests: the E102 chokepoint registration, the
//! ADR-085 §6 status gate (E020), and the W104 readiness walk.

use std::sync::Arc;

use oxigraph::model::{NamedNode, NamedNodeRef};
use oxigraph::store::Store;

use dec_ontology::ontology::archetype::{
    archetype_iri, Archetype, ArchetypeEvidence, ArchetypeStatus, Variance,
};
use dec_ontology::ontology::provenance::Provenance;
use dec_ontology::vocab::orchestration_graph;

use super::promotion::{
    promotion_ready_candidates, validate_status_transition_with_store, StatusWriteAuthority,
    E020_CODE,
};
use super::write::write_archetype;
use crate::stream_writer::StreamWriter;

fn n(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

fn fixture(id: &str, status: ArchetypeStatus, seam_audits: usize) -> Archetype {
    Archetype {
        id: archetype_iri(id),
        title: format!("Archetype {id}"),
        status,
        application_contract: n("https://decision-cli.dev/ns/contract/app/x"),
        infrastructure_contract_template: n("https://decision-cli.dev/ns/contract/infra/x"),
        infrastructure_contract_instances: vec![],
        application_task_types: vec![],
        infrastructure_task_types: vec![],
        archetype_audits: vec![],
        seam_audits: (0..seam_audits)
            .map(|i| n(&format!("https://decision-cli.dev/ns/audit/seam/{id}-{i}")))
            .collect(),
        evidence: ArchetypeEvidence {
            archetype_layer_estimate: 0.5,
            instance_variance: Variance::Low,
            application_contract_held_invariant: true,
            coverage_note: "covers the core flows".to_string(),
        },
        provenance: Provenance {
            was_generated_by: n("https://decision-cli.dev/ns/session/t"),
            was_attributed_to: n("https://decision-cli.dev/ns/agent/t"),
            generated_at_time: "2026-06-11T12:00:00Z".to_string(),
            motivational: vec![],
        },
    }
}

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("store"));
    let stream = n("https://decision-cli.dev/ns/stream/test");
    let writer = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("bootstrap writer");
    (store, writer)
}

fn graph() -> NamedNodeRef<'static> {
    orchestration_graph()
}

/// FT-147 §Behaviour 2 — the chokepoint refuses an archetype with an
/// empty seam-audit set (E102 fires inside StreamWriter::commit).
#[test]
fn chokepoint_refuses_empty_seam_audits_with_e102() {
    let (_store, writer) = writer();
    let archetype = fixture("no-seams", ArchetypeStatus::Candidate, 0);
    let err = write_archetype(&writer, &archetype, graph(), StatusWriteAuthority::Standard)
        .expect_err("empty seam audits must be refused at the chokepoint");
    assert!(err.to_string().contains("E102"), "{err:#}");
}

/// A well-formed candidate archetype commits through the chokepoint.
#[test]
fn chokepoint_accepts_candidate_with_seam_audits() {
    let (store, writer) = writer();
    let archetype = fixture("good", ArchetypeStatus::Candidate, 3);
    write_archetype(&writer, &archetype, graph(), StatusWriteAuthority::Standard)
        .expect("candidate with seam audits commits");
    assert_eq!(promotion_ready_candidates(&store).len(), 1, "W104 fires");
}

/// ADR-085 §6 — minting `standard` outside the promote path is refused
/// with E020; the promote path is allowed.
#[test]
fn status_standard_outside_promote_path_is_refused_with_e020() {
    let (store, writer) = writer();
    let archetype = fixture("gated", ArchetypeStatus::Standard, 3);

    let err = write_archetype(&writer, &archetype, graph(), StatusWriteAuthority::Standard)
        .expect_err("standard outside promote path must be refused");
    assert!(err.to_string().contains(E020_CODE), "{err:#}");

    write_archetype(
        &writer,
        &archetype,
        graph(),
        StatusWriteAuthority::PromotePath,
    )
    .expect("promote path may mint standard");
    drop(store);
}

/// ADR-085 §6 — changing a stored status (candidate → quarantined)
/// outside the promote/demote path is refused with E020.
#[test]
fn status_change_outside_promote_path_is_refused_with_e020() {
    let (store, writer) = writer();
    let mut archetype = fixture("flip", ArchetypeStatus::Candidate, 2);
    write_archetype(&writer, &archetype, graph(), StatusWriteAuthority::Standard)
        .expect("initial candidate registration");

    archetype.status = ArchetypeStatus::Quarantined;
    let quads = archetype.to_quads(graph());
    let err = validate_status_transition_with_store(&store, &quads, StatusWriteAuthority::Standard)
        .expect_err("status change outside promote path must be refused");
    assert!(err.to_string().contains(E020_CODE), "{err}");
}

/// W104 is informational and only fires when the evidence holds.
#[test]
fn w104_skips_candidates_with_weak_evidence() {
    let (store, writer) = writer();
    let mut archetype = fixture("weak", ArchetypeStatus::Candidate, 1);
    archetype.evidence.application_contract_held_invariant = false;
    write_archetype(&writer, &archetype, graph(), StatusWriteAuthority::Standard)
        .expect("candidate registers");
    assert!(promotion_ready_candidates(&store).is_empty());
}
