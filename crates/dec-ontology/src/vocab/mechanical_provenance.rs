//! FT-069 / ADR-038 — universal mechanical-provenance vocabulary.
//!
//! Split out of `core::vocab` per the ADR-013 file-length policy. The
//! constants below are PROV-O standard predicates plus the `dec:` IRIs
//! the universal mechanical-provenance SHACL fragment targets. They are
//! re-exported from the parent module so callers continue to import via
//! `decision_cli::vocab`.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

// --- PROV-O mechanical-provenance predicates --------------------------------
// `IRI_PROV_WAS_ATTRIBUTED_TO` and `IRI_PROV_USED` are re-exported from the
// pre-existing `core::vocab::waiver` module — see `was_attributed_to_pred`
// and `used_pred` below, which build NamedNodeRefs over those constants.
// `IRI_PROV_WAS_INFORMED_BY` lives in `core::vocab` (mod.rs).

/// `prov:wasGeneratedBy` predicate — artifact → producing Session.
pub const IRI_PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";

/// `prov:generatedAtTime` predicate — artifact → wall-clock timestamp.
pub const IRI_PROV_GENERATED_AT_TIME: &str = "http://www.w3.org/ns/prov#generatedAtTime";

/// `prov:wasAssociatedWith` predicate — Session → Agent.
pub const IRI_PROV_WAS_ASSOCIATED_WITH: &str = "http://www.w3.org/ns/prov#wasAssociatedWith";

/// `prov:wasAttributedTo` IRI re-used by the mechanical fragment.
pub const IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL: &str = "http://www.w3.org/ns/prov#wasAttributedTo";

/// `prov:used` IRI re-used by the session-provenance fragment.
pub const IRI_PROV_USED_MECHANICAL: &str = "http://www.w3.org/ns/prov#used";

// --- Domain class IRIs the mechanical fragment targets ----------------------

/// Class IRI for `dec:Agent` (FT-069 / ADR-038).
pub const IRI_DEC_AGENT: &str = "https://decision-cli.dev/ns#Agent";

/// Class IRI for `dec:Artifact` (FT-069 / ADR-038).
pub const IRI_DEC_ARTIFACT: &str = "https://decision-cli.dev/ns#Artifact";

/// `xsd:dateTime` datatype IRI.
pub const IRI_XSD_DATE_TIME: &str = "http://www.w3.org/2001/XMLSchema#dateTime";

// --- NamedNodeRef accessors -------------------------------------------------

#[must_use]
pub fn was_generated_by_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_WAS_GENERATED_BY)
}

#[must_use]
pub fn was_attributed_to_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL)
}

#[must_use]
pub fn generated_at_time_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_GENERATED_AT_TIME)
}

#[must_use]
pub fn used_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_USED_MECHANICAL)
}

#[must_use]
pub fn was_associated_with_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_WAS_ASSOCIATED_WITH)
}

#[must_use]
pub fn agent_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_AGENT)
}

#[must_use]
pub fn artifact_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ARTIFACT)
}
