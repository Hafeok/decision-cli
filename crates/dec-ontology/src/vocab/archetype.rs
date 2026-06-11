//! FT-147 / ADR-082 — `dec:Archetype` vocabulary.
//!
//! The catalog layer above the TaskType + Cell substrate (ADR-080).
//! Predicates use the canonical `https://decision-cli.dev/ns#` namespace
//! shared by every other artifact type; archetype instances live under
//! the `…/ns/archetype/<archetype-id>` IRI prefix.

/// Class IRI for `dec:Archetype` (ADR-082).
pub const IRI_DEC_ARCHETYPE: &str = "https://decision-cli.dev/ns#Archetype";

/// IRI prefix for archetype instances (`…/ns/archetype/<archetype-id>`).
pub const IRI_DEC_ARCHETYPE_PREFIX: &str = "https://decision-cli.dev/ns/archetype/";

/// `dec:title` predicate on an archetype.
pub const IRI_DEC_ARCHETYPE_TITLE: &str = "https://decision-cli.dev/ns#title";
/// `dec:status` predicate — `candidate | standard | quarantined` (ADR-085).
pub const IRI_DEC_ARCHETYPE_STATUS: &str = "https://decision-cli.dev/ns#status";
/// `dec:applicationContract` predicate — → ApplicationContract IRI (FT-148).
pub const IRI_DEC_APPLICATION_CONTRACT: &str = "https://decision-cli.dev/ns#applicationContract";
/// `dec:infrastructureContractTemplate` predicate (FT-149).
pub const IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE: &str =
    "https://decision-cli.dev/ns#infrastructureContractTemplate";
/// `dec:infrastructureContractInstance` predicate (FT-149, repeatable).
pub const IRI_DEC_INFRASTRUCTURE_CONTRACT_INSTANCE: &str =
    "https://decision-cli.dev/ns#infrastructureContractInstance";
/// `dec:applicationTaskType` predicate — → TaskType IRI with family=application.
pub const IRI_DEC_APPLICATION_TASK_TYPE: &str = "https://decision-cli.dev/ns#applicationTaskType";
/// `dec:infrastructureTaskType` predicate — → TaskType IRI with family=infrastructure.
pub const IRI_DEC_INFRASTRUCTURE_TASK_TYPE: &str =
    "https://decision-cli.dev/ns#infrastructureTaskType";
/// `dec:archetypeAudit` predicate — → ArchetypeAudit IRI (FT-152, repeatable).
pub const IRI_DEC_ARCHETYPE_AUDIT: &str = "https://decision-cli.dev/ns#archetypeAudit";
/// `dec:seamAudit` predicate — → SeamAudit IRI. Non-empty per ADR-084 §1 (E102).
pub const IRI_DEC_SEAM_AUDIT: &str = "https://decision-cli.dev/ns#seamAudit";

/// `dec:archetypeLayerEstimate` evidence predicate — xsd:float.
pub const IRI_DEC_ARCHETYPE_LAYER_ESTIMATE: &str =
    "https://decision-cli.dev/ns#archetypeLayerEstimate";
/// `dec:instanceVariance` evidence predicate — `low | medium | high`.
pub const IRI_DEC_INSTANCE_VARIANCE: &str = "https://decision-cli.dev/ns#instanceVariance";
/// `dec:applicationContractHeldInvariant` evidence predicate — xsd:boolean.
pub const IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT: &str =
    "https://decision-cli.dev/ns#applicationContractHeldInvariant";
/// `dec:coverageNote` evidence predicate — free-text coverage honesty (ADR-085 §1.3).
pub const IRI_DEC_COVERAGE_NOTE: &str = "https://decision-cli.dev/ns#coverageNote";

/// Allowed `dec:status` literal values (ADR-085).
pub const ARCHETYPE_STATUS_VALUES: &[&str] = &["candidate", "standard", "quarantined"];

/// Allowed `dec:instanceVariance` literal values (ADR-082 §3).
pub const ARCHETYPE_VARIANCE_VALUES: &[&str] = &["low", "medium", "high"];
