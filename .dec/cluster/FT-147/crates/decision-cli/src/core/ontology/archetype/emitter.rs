use crate::core::ontology::archetype::{Archetype, ArchetypeStatus, Variance, Provenance, ProvenanceMechanical, ProvenanceMotivational, ArchetypeEvidence};
use crate::core::ontology::vocab::archetype as vocab;
use crate::core::ontology::vocab::prov as prov_vocab;
use crate::core::ontology::vocab::motivational as motivation_vocab;
use crate::core::ontology::vocab::task_type as task_type_vocab;
use crate::core::ontology::vocab::contract as contract_vocab;
use crate::core::ontology::vocab::audit as audit_vocab;
use crate::core::ontology::vocab::evidence as evidence_vocab;
use crate::core::ontology::vocab::provenance as provenance_vocab;
use oxigraph::model::{NamedNode, Quad, Subject, GraphName};

pub fn emit_archetype(archetype: &Archetype) -> Vec<Quad> {
    let mut quads = Vec::new();
    let subject = archetype.id.clone();

    // Basic properties
    quads.push(Quad::new(
        subject.clone(),
        vocab::ARCHETYPE_CLASS.into(),
        NamedNode::new("https://decisionframework.org/ns/Archetype").unwrap().into(),
        None,
    ));

    quads.push(Quad::new(
        subject.clone(),
        vocab::ARCHETYPE_TITLE.into(),
        archetype.title.clone().into(),
        None,
    ));

    // Status
    let status_str = match archetype.status {
        ArchetypeStatus::Candidate => "candidate",
        ArchetypeStatus::Standard => "standard",
        ArchetypeStatus::Quarantined => "quarantined",
    };
    quads.push(Quad::new(
        subject.clone(),
        vocab::ARCHETYPE_STATUS.into(),
        status_str.into(),
        None,
    ));

    // Contract links
    quads.push(Quad::new(
        subject.clone(),
        contract_vocab::APPLICATION_CONTRACT.into(),
        archetype.application_contract.clone().into(),
        None,
    ));

    quads.push(Quad::new(
        subject.clone(),
        contract_vocab::INFRASTRUCTURE_CONTRACT_TEMPLATE.into(),
        archetype.infrastructure_contract_template.clone().into(),
        None,
    ));

    // Infrastructure contract instances
    for instance in &archetype.infrastructure_contract_instances {
        quads.push(Quad::new(
            subject.clone(),
            contract_vocab::INFRASTRUCTURE_CONTRACT_INSTANCE.into(),
            instance.clone().into(),
            None,
        ));
    }

    // Task type links
    for task_type in &archetype.application_task_types {
        quads.push(Quad::new(
            subject.clone(),
            task_type_vocab::APPLICATION_TASK_TYPE.into(),
            task_type.clone().into(),
            None,
        ));
    }

    for task_type in &archetype.infrastructure_task_types {
        quads.push(Quad::new(
            subject.clone(),
            task_type_vocab::INFRASTRUCTURE_TASK_TYPE.into(),
            task_type.clone().into(),
            None,
        ));
    }

    // Audit links
    for audit in &archetype.archetype_audits {
        quads.push(Quad::new(
            subject.clone(),
            audit_vocab::ARCHETYPE_AUDIT.into(),
            audit.clone().into(),
            None,
        ));
    }

    for audit in &archetype.seam_audits {
        quads.push(Quad::new(
            subject.clone(),
            audit_vocab::SEAM_AUDIT.into(),
            audit.clone().into(),
            None,
        ));
    }

    // Evidence
    quads.push(Quad::new(
        subject.clone(),
        evidence_vocab::ARCHETYPE_LAYER_ESTIMATE.into(),
        archetype.evidence.archetype_layer_estimate.to_string().into(),
        None,
    ));

    let variance_str = match archetype.evidence.instance_variance {
        Variance::Low => "low",
        Variance::Medium => "medium",
        Variance::High => "high",
    };
    quads.push(Quad::new(
        subject.clone(),
        evidence_vocab::INSTANCE_VARIANCE.into(),
        variance_str.into(),
        None,
    ));

    quads.push(Quad::new(
        subject.clone(),
        evidence_vocab::APPLICATION_CONTRACT_HELD_INVARIANT.into(),
        archetype.evidence.application_contract_held_invariant.to_string().into(),
        None,
    ));

    quads.push(Quad::new(
        subject.clone(),
        evidence_vocab::COVERAGE_NOTE.into(),
        archetype.evidence.coverage_note.clone().into(),
        None,
    ));

    // Provenance - Mechanical
    if let Some(mechanical) = &archetype.provenance.mechanical {
        quads.push(Quad::new(
            subject.clone(),
            provenance_vocab::MECHANICAL_GENERATED_BY.into(),
            mechanical.generated_by.clone().into(),
            None,
        ));

        quads.push(Quad::new(
            subject.clone(),
            provenance_vocab::MECHANICAL_GENERATED_AT.into(),
            mechanical.generated_at.clone().into(),
            None,
        ));

        quads.push(Quad::new(
            subject.clone(),
            provenance_vocab::MECHANICAL_GENERATED_VIA.into(),
            mechanical.generated_via.clone().into(),
            None,
        ));
    }

    // Provenance - Motivational
    if let Some(motivational) = &archetype.provenance.motivational {
        quads.push(Quad::new(
            subject.clone(),
            provenance_vocab::MOTIVATIONAL_MOTIVATED_BY.into(),
            motivational.motivated_by.clone().into(),
            None,
        ));

        quads.push(Quad::new(
            subject.clone(),
            provenance_vocab::MOTIVATIONAL_MOTIVATED_VIA.into(),
            motivational.motivated_via.clone().into(),
            None,
        ));
    }

    quads
}