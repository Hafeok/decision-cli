//! Emitter for Archetype artifact type.

use crate::ontology::archetype::{Archetype, ArchetypeStatus, ArchetypeEvidence, Variance, Provenance};
use crate::vocab::archetype::*;
use oxigraph::model::{NamedNode, Quad, Subject, Literal, Graph};

/// Emit an Archetype as a collection of quads.
pub fn emit_archetype(archetype: &Archetype) -> Vec<Quad> {
    let mut quads = Vec::new();
    
    // Add archetype class
    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_CLASS.clone(),
        Literal::from(true).into(),
    ));
    
    // Add title
    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_TITLE.clone(),
        Literal::from(archetype.title.clone()).into(),
    ));
    
    // Add status
    let status_iri = match archetype.status {
        ArchetypeStatus::Candidate => CANDIDATE_STATUS.clone(),
        ArchetypeStatus::Standard => STANDARD_STATUS.clone(),
        ArchetypeStatus::Quarantined => QUARANTINED_STATUS.clone(),
    };
    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_STATUS.clone(),
        status_iri.into(),
    ));
    
    // Add application contract
    quads.push(Quad::new(
        archetype.id.clone(),
        APPLICATION_CONTRACT.clone(),
        archetype.application_contract.clone().into(),
    ));
    
    // Add infrastructure contract template
    quads.push(Quad::new(
        archetype.id.clone(),
        INFRASTRUCTURE_CONTRACT_TEMPLATE.clone(),
        archetype.infrastructure_contract_template.clone().into(),
    ));
    
    // Add infrastructure contract instances
    for instance in &archetype.infrastructure_contract_instances {
        quads.push(Quad::new(
            archetype.id.clone(),
            INFRASTRUCTURE_CONTRACT_INSTANCES.clone(),
            instance.clone().into(),
        ));
    }
    
    // Add application task types
    for task_type in &archetype.application_task_types {
        quads.push(Quad::new(
            archetype.id.clone(),
            APPLICATION_TASK_TYPES.clone(),
            task_type.clone().into(),
        ));
    }
    
    // Add infrastructure task types
    for task_type in &archetype.infrastructure_task_types {
        quads.push(Quad::new(
            archetype.id.clone(),
            INFRASTRUCTURE_TASK_TYPES.clone(),
            task_type.clone().into(),
        ));
    }
    
    // Add archetype audits
    for audit in &archetype.archetype_audits {
        quads.push(Quad::new(
            archetype.id.clone(),
            ARCHETYPE_AUDITS.clone(),
            audit.clone().into(),
        ));
    }
    
    // Add seam audits
    for audit in &archetype.seam_audits {
        quads.push(Quad::new(
            archetype.id.clone(),
            SEAM_AUDITS.clone(),
            audit.clone().into(),
        ));
    }
    
    // Add evidence
    let evidence_node = NamedNode::new_unchecked(format!("{}_evidence", archetype.id.as_str()).as_str());
    quads.push(Quad::new(
        archetype.id.clone(),
        ARCHETYPE_EVIDENCE.clone(),
        evidence_node.clone().into(),
    ));
    
    // Add evidence details
    quads.push(Quad::new(
        evidence_node.clone(),
        ARCHETYPE_LAYER_ESTIMATE.clone(),
        Literal::from(archetype.evidence.archetype_layer_estimate).into(),
    ));
    
    let variance_iri = match archetype.evidence.instance_variance {
        Variance::Low => LOW_VARIANCE.clone(),
        Variance::Medium => MEDIUM_VARIANCE.clone(),
        Variance::High => HIGH_VARIANCE.clone(),
    };
    quads.push(Quad::new(
        evidence_node.clone(),
        INSTANCE_VARIANCE.clone(),
        variance_iri.into(),
    ));
    
    quads.push(Quad::new(
        evidence_node.clone(),
        APPLICATION_CONTRACT_HELD_INVARIANT.clone(),
        Literal::from(archetype.evidence.application_contract_held_invariant).into(),
    ));
    
    quads.push(Quad::new(
        evidence_node.clone(),
        COVERAGE_NOTE.clone(),
        Literal::from(archetype.evidence.coverage_note.clone()).into(),
    ));
    
    // Add provenance
    let provenance_node = NamedNode::new_unchecked(format!("{}_provenance", archetype.id.as_str()).as_str());
    quads.push(Quad::new(
        archetype.id.clone(),
        PROVENANCE.clone(),
        provenance_node.clone().into(),
    ));
    
    // Add provenance details
    quads.push(Quad::new(
        provenance_node.clone(),
        MECHANICAL_PROVENANCE.clone(),
        archetype.provenance.mechanical.clone().into(),
    ));
    
    quads.push(Quad::new(
        provenance_node.clone(),
        MOTIVATIONAL_PROVENANCE.clone(),
        archetype.provenance.motivational.clone().into(),
    ));
    
    quads
}