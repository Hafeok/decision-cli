//! IRI vocabulary for archetype artifact type.

use oxigraph::model::NamedNode;

/// Class IRI for Archetype.
pub const ARCHETYPE_CLASS: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#Archetype");

/// Property IRI for archetype title.
pub const ARCHETYPE_TITLE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#archetypeTitle");

/// Property IRI for archetype status.
pub const ARCHETYPE_STATUS: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#archetypeStatus");

/// Property IRI for application contract.
pub const APPLICATION_CONTRACT: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#applicationContract");

/// Property IRI for infrastructure contract template.
pub const INFRASTRUCTURE_CONTRACT_TEMPLATE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#infrastructureContractTemplate");

/// Property IRI for infrastructure contract instances.
pub const INFRASTRUCTURE_CONTRACT_INSTANCES: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#infrastructureContractInstances");

/// Property IRI for application task types.
pub const APPLICATION_TASK_TYPES: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#applicationTaskTypes");

/// Property IRI for infrastructure task types.
pub const INFRASTRUCTURE_TASK_TYPES: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#infrastructureTaskTypes");

/// Property IRI for archetype audits.
pub const ARCHETYPE_AUDITS: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#archetypeAudits");

/// Property IRI for seam audits.
pub const SEAM_AUDITS: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#seamAudits");

/// Property IRI for archetype evidence.
pub const ARCHETYPE_EVIDENCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#archetypeEvidence");

/// Property IRI for provenance.
pub const PROVENANCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#provenance");

/// Property IRI for archetype ID.
pub const ARCHETYPE_ID: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#archetypeId");

/// Value IRI for candidate status.
pub const CANDIDATE_STATUS: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#candidate");

/// Value IRI for standard status.
pub const STANDARD_STATUS: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#standard");

/// Value IRI for quarantined status.
pub const QUARANTINED_STATUS: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#quarantined");

/// Property IRI for archetype layer estimate.
pub const ARCHETYPE_LAYER_ESTIMATE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#archetypeLayerEstimate");

/// Property IRI for instance variance.
pub const INSTANCE_VARIANCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#instanceVariance");

/// Property IRI for application contract held invariant.
pub const APPLICATION_CONTRACT_HELD_INVARIANT: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#applicationContractHeldInvariant");

/// Property IRI for coverage note.
pub const COVERAGE_NOTE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#coverageNote");

/// Value IRI for low variance.
pub const LOW_VARIANCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#low");

/// Value IRI for medium variance.
pub const MEDIUM_VARIANCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#medium");

/// Value IRI for high variance.
pub const HIGH_VARIANCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#high");

/// Property IRI for mechanical provenance.
pub const MECHANICAL_PROVENANCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#mechanicalProvenance");

/// Property IRI for motivational provenance.
pub const MOTIVATIONAL_PROVENANCE: NamedNode = NamedNode::new_unchecked("https://decision-archetype.org/ns/dec#motivationalProvenance");