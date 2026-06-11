use crate::ontology::provenance::Provenance;
use crate::vocab::archetype::*;
use oxigraph::model::{NamedNode, Quad};

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

impl From<&str> for Variance {
    fn from(s: &str) -> Self {
        match s {
            "low" => Variance::Low,
            "medium" => Variance::Medium,
            "high" => Variance::High,
            _ => panic!("Invalid variance value"),
        }
    }
}

impl From<Variance> for String {
    fn from(v: Variance) -> Self {
        match v {
            Variance::Low => "low".to_string(),
            Variance::Medium => "medium".to_string(),
            Variance::High => "high".to_string(),
        }
    }
}

impl Archetype {
    pub fn to_quads(&self) -> Vec<Quad> {
        let mut quads = Vec::new();

        quads.push(Quad::new(
            self.id.clone(),
            ARCHETYPE_TITLE.into(),
            self.title.clone().into(),
            None,
        ));

        quads.push(Quad::new(
            self.id.clone(),
            ARCHETYPE_STATUS.into(),
            match self.status {
                ArchetypeStatus::Candidate => "candidate",
                ArchetypeStatus::Standard => "standard",
                ArchetypeStatus::Quarantined => "quarantined",
            }
            .into(),
            None,
        ));

        quads.push(Quad::new(
            self.id.clone(),
            APPLICATION_CONTRACT.into(),
            self.application_contract.clone(),
            None,
        ));

        quads.push(Quad::new(
            self.id.clone(),
            INFRASTRUCTURE_CONTRACT_TEMPLATE.into(),
            self.infrastructure_contract_template.clone(),
            None,
        ));

        for instance in &self.infrastructure_contract_instances {
            quads.push(Quad::new(
                self.id.clone(),
                INFRASTRUCTURE_CONTRACT_INSTANCES.into(),
                instance.clone(),
                None,
            ));
        }

        for task_type in &self.application_task_types {
            quads.push(Quad::new(
                self.id.clone(),
                APPLICATION_TASK_TYPES.into(),
                task_type.clone(),
                None,
            ));
        }

        for task_type in &self.infrastructure_task_types {
            quads.push(Quad::new(
                self.id.clone(),
                INFRASTRUCTURE_TASK_TYPES.into(),
                task_type.clone(),
                None,
            ));
        }

        for audit in &self.archetype_audits {
            quads.push(Quad::new(
                self.id.clone(),
                ARCHETYPE_AUDITS.into(),
                audit.clone(),
                None,
            ));
        }

        for audit in &self.seam_audits {
            quads.push(Quad::new(
                self.id.clone(),
                SEAM_AUDITS.into(),
                audit.clone(),
                None,
            ));
        }

        quads.push(Quad::new(
            self.id.clone(),
            ARCHETYPE_LAYER_ESTIMATE.into(),
            self.evidence.archetype_layer_estimate.to_string().into(),
            None,
        ));

        quads.push(Quad::new(
            self.id.clone(),
            INSTANCE_VARIANCE.into(),
            String::from(self.evidence.instance_variance).into(),
            None,
        ));

        quads.push(Quad::new(
            self.id.clone(),
            APPLICATION_CONTRACT_HELD_INVARIANT.into(),
            self.evidence.application_contract_held_invariant.to_string().into(),
            None,
        ));

        quads.push(Quad::new(
            self.id.clone(),
            COVERAGE_NOTE.into(),
            self.evidence.coverage_note.clone().into(),
            None,
        ));

        quads.extend(Provenance::to_quads(&self.provenance, &self.id));

        quads
    }
}