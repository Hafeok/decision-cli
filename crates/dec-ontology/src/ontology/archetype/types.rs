//! In-memory shape of `dec:Archetype` (FT-147 / ADR-082 §2).

use oxrdf::NamedNode;

use crate::ontology::provenance::Provenance;
use crate::vocab::IRI_DEC_ARCHETYPE_PREFIX;

/// Build the canonical instance IRI for an archetype id
/// (`…/ns/archetype/<archetype-id>`).
#[must_use]
pub fn archetype_iri(archetype_id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{IRI_DEC_ARCHETYPE_PREFIX}{archetype_id}"))
}

/// The catalog layer above TaskType (ADR-082): one recurring kind of
/// system with its contracts, TaskType set, audits, and evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Archetype {
    /// Instance IRI (`…/ns/archetype/<archetype-id>`).
    pub id: NamedNode,
    /// Human-readable name of the system kind.
    pub title: String,
    /// Lifecycle status — promotion is human-gated (ADR-085).
    pub status: ArchetypeStatus,
    /// → ApplicationContract IRI (FT-148).
    pub application_contract: NamedNode,
    /// → InfrastructureContractTemplate IRI (FT-149).
    pub infrastructure_contract_template: NamedNode,
    /// → InfrastructureContractInstance IRIs (FT-149).
    pub infrastructure_contract_instances: Vec<NamedNode>,
    /// → TaskType IRIs with family=application.
    pub application_task_types: Vec<NamedNode>,
    /// → TaskType IRIs with family=infrastructure.
    pub infrastructure_task_types: Vec<NamedNode>,
    /// → ArchetypeAudit IRIs (FT-152).
    pub archetype_audits: Vec<NamedNode>,
    /// → SeamAudit IRIs — must be non-empty per ADR-084 §1 (E102).
    pub seam_audits: Vec<NamedNode>,
    /// Coverage estimate, variance, contract invariance (ADR-085 §1).
    pub evidence: ArchetypeEvidence,
    /// Mechanical + motivational provenance (FT-072 / FT-073).
    pub provenance: Provenance,
}

/// `dec:status` controlled vocabulary (ADR-085).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchetypeStatus {
    /// Mined or directly authored; still validating.
    Candidate,
    /// Promotion is a gated human decision with evidence (ADR-085).
    Standard,
    /// Withdrawn from recommendation.
    Quarantined,
}

impl ArchetypeStatus {
    /// Canonical literal value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Standard => "standard",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parse the canonical literal value.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "candidate" => Some(Self::Candidate),
            "standard" => Some(Self::Standard),
            "quarantined" => Some(Self::Quarantined),
            _ => None,
        }
    }
}

/// `dec:instanceVariance` controlled vocabulary (ADR-082 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    /// Instances barely deviate from the archetype layer.
    Low,
    /// Moderate per-instance deviation.
    Medium,
    /// High deviation — the archetype-layer estimate is weak evidence.
    High,
}

impl Variance {
    /// Canonical literal value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Parse the canonical literal value.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

/// The EVIDENCE block (ADR-085 §1.3 coverage honesty).
#[derive(Debug, Clone, PartialEq)]
pub struct ArchetypeEvidence {
    /// Fraction of instance code the archetype layer covers (0.0–1.0).
    pub archetype_layer_estimate: f32,
    /// Observed deviation across known instances.
    pub instance_variance: Variance,
    /// Whether the application contract held without per-instance edits.
    pub application_contract_held_invariant: bool,
    /// Free-text honesty note about what the estimate covers.
    pub coverage_note: String,
}
