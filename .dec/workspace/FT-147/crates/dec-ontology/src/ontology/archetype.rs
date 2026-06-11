//! Archetype artifact type definition.

use oxigraph::model::{NamedNode, Quad};

/// An archetype is the unit of cross-customer reuse: a recurring *kind of system*
/// (Self-Service Portal, Internal Admin Tool, Approval Workflow).
///
/// It owns two parallel contracts (application + infrastructure), a TaskType set split by family,
/// and audits at three scopes.
#[derive(Debug, Clone, PartialEq)]
pub struct Archetype {
    /// Unique identifier for this archetype.
    pub id: NamedNode,
    
    /// Human-readable title of the archetype.
    pub title: String,
    
    /// Status of the archetype.
    pub status: ArchetypeStatus,
    
    /// Link to the application contract for this archetype.
    pub application_contract: NamedNode,
    
    /// Link to the infrastructure contract template for this archetype.
    pub infrastructure_contract_template: NamedNode,
    
    /// Set of infrastructure contract instances for this archetype.
    pub infrastructure_contract_instances: Vec<NamedNode>,
    
    /// Set of application TaskTypes for this archetype.
    pub application_task_types: Vec<NamedNode>,
    
    /// Set of infrastructure TaskTypes for this archetype.
    pub infrastructure_task_types: Vec<NamedNode>,
    
    /// Set of archetype audits for this archetype.
    pub archetype_audits: Vec<NamedNode>,
    
    /// Set of seam audits for this archetype.
    pub seam_audits: Vec<NamedNode>,
    
    /// Evidence about the archetype.
    pub evidence: ArchetypeEvidence,
    
    /// Provenance information for the archetype.
    pub provenance: Provenance,
}

/// Status of an archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchetypeStatus {
    /// Candidate archetype that has not been reviewed for promotion.
    Candidate,
    
    /// Standard archetype that has been reviewed and approved for use.
    Standard,
    
    /// Quarantined archetype that has issues and should not be used.
    Quarantined,
}

/// Evidence about an archetype.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchetypeEvidence {
    /// Estimated fraction of the archetype's features that are covered by the TaskType set.
    pub archetype_layer_estimate: f32,
    
    /// Variance in the instances of this archetype.
    pub instance_variance: Variance,
    
    /// Whether the application contract has held invariant across instances.
    pub application_contract_held_invariant: bool,
    
    /// Additional notes about the coverage.
    pub coverage_note: String,
}

/// Variance level for archetype instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variance {
    /// Low variance.
    Low,
    
    /// Medium variance.
    Medium,
    
    /// High variance.
    High,
}

/// Provenance information for an artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct Provenance {
    /// Mechanical provenance (PROV-O `wasGeneratedBy` etc.).
    pub mechanical: NamedNode,
    
    /// Motivational provenance (predicate vocabulary).
    pub motivational: NamedNode,
}