use crate::core::ontology::vocab::archetype as vocab;
use crate::core::ontology::vocab::prov as prov_vocab;
use crate::core::ontology::vocab::motivational as motivation_vocab;
use crate::core::ontology::vocab::task_type as task_type_vocab;
use crate::core::ontology::vocab::contract as contract_vocab;
use crate::core::ontology::vocab::audit as audit_vocab;
use crate::core::ontology::vocab::evidence as evidence_vocab;
use crate::core::ontology::vocab::provenance as provenance_vocab;
use oxigraph::model::{NamedNode, Quad, Subject, GraphName};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archetype {
    pub id: NamedNode,
    pub title: String,
    pub status: ArchetypeStatus,
    pub application_contract: NamedNode,
    pub infrastructure_contract_template: NamedNode,
    pub infrastructure_contract_instances: Vec<NamedNode>,
    pub application_task_types: Vec<NamedNode>,
    pub infrastructure_task_types: Vec<NamedNode>,
    pub archetype_audits: Vec<NamedNode>,
    pub seam_audits: Vec<NamedNode>,
    pub evidence: ArchetypeEvidence,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchetypeStatus {
    Candidate,
    Standard,
    Quarantined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchetypeEvidence {
    pub archetype_layer_estimate: f32,
    pub instance_variance: Variance,
    pub application_contract_held_invariant: bool,
    pub coverage_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Variance {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub mechanical: Option<ProvenanceMechanical>,
    pub motivational: Option<ProvenanceMotivational>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceMechanical {
    pub generated_by: NamedNode,
    pub generated_at: String,
    pub generated_via: NamedNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceMotivational {
    pub motivated_by: NamedNode,
    pub motivated_via: NamedNode,
}

impl Archetype {
    pub fn new(
        id: NamedNode,
        title: String,
        status: ArchetypeStatus,
        application_contract: NamedNode,
        infrastructure_contract_template: NamedNode,
        infrastructure_contract_instances: Vec<NamedNode>,
        application_task_types: Vec<NamedNode>,
        infrastructure_task_types: Vec<NamedNode>,
        archetype_audits: Vec<NamedNode>,
        seam_audits: Vec<NamedNode>,
        evidence: ArchetypeEvidence,
        provenance: Provenance,
    ) -> Self {
        Self {
            id,
            title,
            status,
            application_contract,
            infrastructure_contract_template,
            infrastructure_contract_instances,
            application_task_types,
            infrastructure_task_types,
            archetype_audits,
            seam_audits,
            evidence,
            provenance,
        }
    }
}