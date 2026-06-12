//! FT-148 / ADR-082 — `dec:ApplicationContract` + `dec:Convention`
//! vocabulary.
//!
//! The application half of an archetype's two parallel contracts: six
//! required conventions plus cross-cutting entries, each a checkable
//! sub-resource. Canonical `https://decision-cli.dev/ns#` namespace.

/// Class IRI for `dec:ApplicationContract` (ADR-082).
pub const IRI_DEC_APPLICATION_CONTRACT_CLASS: &str =
    "https://decision-cli.dev/ns#ApplicationContract";

/// IRI prefix for contract instances (`…/ns/contract/app/<id>`).
pub const IRI_DEC_APPLICATION_CONTRACT_PREFIX: &str = "https://decision-cli.dev/ns/contract/app/";

/// Class IRI for `dec:Convention` — a contract's checkable sub-resource.
pub const IRI_DEC_CONVENTION_CLASS: &str = "https://decision-cli.dev/ns#Convention";

/// `dec:archetype` back-reference predicate — → owning Archetype.
pub const IRI_DEC_CONTRACT_ARCHETYPE: &str = "https://decision-cli.dev/ns#archetype";
/// `dec:languageRuntime` predicate — → Convention.
pub const IRI_DEC_LANGUAGE_RUNTIME: &str = "https://decision-cli.dev/ns#languageRuntime";
/// `dec:layeringRule` predicate — → Convention.
pub const IRI_DEC_LAYERING_RULE: &str = "https://decision-cli.dev/ns#layeringRule";
/// `dec:featureOrganisation` predicate — → Convention.
pub const IRI_DEC_FEATURE_ORGANISATION: &str = "https://decision-cli.dev/ns#featureOrganisation";
/// `dec:persistenceModel` predicate — → Convention.
pub const IRI_DEC_PERSISTENCE_MODEL: &str = "https://decision-cli.dev/ns#persistenceModel";
/// `dec:endpointConvention` predicate — → Convention.
pub const IRI_DEC_ENDPOINT_CONVENTION: &str = "https://decision-cli.dev/ns#endpointConvention";
/// `dec:crossCutting` predicate — → Convention (repeatable).
pub const IRI_DEC_CROSS_CUTTING: &str = "https://decision-cli.dev/ns#crossCutting";

/// `dec:conventionName` predicate — xsd:string.
pub const IRI_DEC_CONVENTION_NAME: &str = "https://decision-cli.dev/ns#conventionName";
/// `dec:conventionBodyPath` predicate — repo-relative path literal.
pub const IRI_DEC_CONVENTION_BODY_PATH: &str = "https://decision-cli.dev/ns#conventionBodyPath";
/// `dec:conventionAuditId` predicate — → ArchetypeAudit IRI (optional).
pub const IRI_DEC_CONVENTION_AUDIT_ID: &str = "https://decision-cli.dev/ns#conventionAuditId";
/// `dec:conventionCheckable` predicate — xsd:boolean. `false` flags
/// dependent TaskTypes `not-safely-dispatchable` (FT-150/FT-153).
pub const IRI_DEC_CONVENTION_CHECKABLE: &str = "https://decision-cli.dev/ns#conventionCheckable";
