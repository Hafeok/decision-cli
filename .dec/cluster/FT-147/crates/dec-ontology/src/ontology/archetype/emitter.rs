use crate::ontology::archetype::{Archetype, ArchetypeStatus, Variance};
use crate::ontology::provenance::Provenance;
use crate::vocab::archetype::*;
use oxigraph::model::{NamedNode, Quad, Subject};

pub fn archetype_to_quads(archetype: &Archetype) -> Vec<Quad> {
    let mut quads = Vec::new();

    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_CLASS.into(),
        ().into(),
        None,
    ));

    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_TITLE.into(),
        archetype.title.clone().into(),
        None,
    ));

    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_STATUS.into(),
        match archetype.status {
            ArchetypeStatus::Candidate => "candidate".into(),
            ArchetypeStatus::Standard => "standard".into(),
            ArchetypeStatus::Quarantined => "quarantined".into(),
        },
        None,
    ));

    quads.push(Quad::new(
        archetype.id.clone(),
        APPLICATION_CONTRACT.into(),
        archetype.application_contract.clone(),
        None,
    ));

    quads.push(Quad::new(
        archetype.id.clone(),
        INFRASTRUCTURE_CONTRACT_TEMPLATE.into(),
        archetype.infrastructure_contract_template.clone(),
        None,
    ));

    for instance in &archetype.infrastructure_contract_instances {
        quads.push(Quad::new(
            archetype.id.clone(),
            INFRASTRUCTURE_CONTRACT_INSTANCES.into(),
            instance.clone(),
            None,
        ));
    }

    for task_type in &archetype.application_task_types {
        quads.push(Quad::new(
            archetype.id.clone(),
            APPLICATION_TASK_TYPES.into(),
            task_type.clone(),
            None,
        ));
    }

    for task_type in &archetype.infrastructure_task_types {
        quads.push(Quad::new(
            archetype.id.clone(),
            INFRASTRUCTURE_TASK_TYPES.into(),
            task_type.clone(),
            None,
        ));
    }

    for audit in &archetype.archetype_audits {
        quads.push(Quad::new(
            archetype.id.clone(),
            ARCHETYPE_AUDITS.into(),
            audit.clone(),
            None,
        ));
    }

    for audit in &archetype.seam_audits {
        quads.push(Quad::new(
            archetype.id.clone(),
            SEAM_AUDITS.into(),
            audit.clone(),
            None,
        ));
    }

    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_LAYER_ESTIMATE.into(),
        archetype.evidence.archetype_layer_estimate.to_string().into(),
        None,
    ));

    quads.push(Quad::new(
        archetype.id.clone(),
        INSTANCE_VARIANCE.into(),
        String::from(archetype.evidence.instance_variance).into(),
        None,
    ));

    quads.push(Quad::new(
        archetype.id.clone(),
        APPLICATION_CONTRACT_HELD_INVARIANT.into(),
        archetype.evidence.application_contract_held_invariant.to_string().into(),
        None,
    ));

    quads.push(Quad::new(
        archetype.id.clone(),
        COVERAGE_NOTE.into(),
        archetype.evidence.coverage_note.clone().into(),
        None,
    ));

    quads.extend(Provenance::to_quads(&archetype.provenance, &archetype.id));

    quads
}