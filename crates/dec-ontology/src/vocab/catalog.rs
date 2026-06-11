//! FT-101 — catalog artifact vocabulary (CapabilityReference,
//! OntologyDescription, ExemplarGraph).
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! imports IRIs from this module. The three artifact types share enough
//! shape that a single vocab file is the right grain.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

// --- Class IRIs -------------------------------------------------------------

/// `dec:CapabilityReference` — structured reference for a single `dec`
/// subcommand the worker may invoke from a `shell-command` step.
pub const IRI_DEC_CAPABILITY_REFERENCE: &str = "https://decision-cli.dev/ns#CapabilityReference";

/// `dec:OntologyDescription` — declarative description of the dec
/// namespace, classes, and canonical predicates.
pub const IRI_DEC_ONTOLOGY_DESCRIPTION: &str = "https://decision-cli.dev/ns#OntologyDescription";

/// `dec:ExemplarGraph` — reference to a known-good
/// `dec:VerificationGraph` tagged for an env's `safety_class`.
pub const IRI_DEC_EXEMPLAR_GRAPH: &str = "https://decision-cli.dev/ns#ExemplarGraph";

// --- Predicate IRIs ---------------------------------------------------------

/// `dec:command` — CapabilityReference: the canonical command string.
pub const IRI_DEC_COMMAND_CR: &str = "https://decision-cli.dev/ns#command";

/// `dec:capabilityVersion` — CapabilityReference: semver string for the
/// dec release the reference describes.
pub const IRI_DEC_CAPABILITY_VERSION_CR: &str = "https://decision-cli.dev/ns#capabilityVersion";

/// `dec:capabilityBody` — CapabilityReference: opaque JSON literal.
pub const IRI_DEC_CAPABILITY_BODY: &str = "https://decision-cli.dev/ns#capabilityBody";

/// `dec:namespace` — OntologyDescription: IRI string of the dec namespace.
pub const IRI_DEC_NAMESPACE: &str = "https://decision-cli.dev/ns#namespace";

/// `dec:prefix` — OntologyDescription: prefix alias (usually `"dec"`).
pub const IRI_DEC_PREFIX: &str = "https://decision-cli.dev/ns#prefix";

/// `dec:ontologyVersion` — OntologyDescription: semver string.
pub const IRI_DEC_ONTOLOGY_VERSION: &str = "https://decision-cli.dev/ns#ontologyVersion";

/// `dec:ontologyBody` — OntologyDescription: opaque JSON literal.
pub const IRI_DEC_ONTOLOGY_BODY: &str = "https://decision-cli.dev/ns#ontologyBody";

/// `dec:exemplarOf` — ExemplarGraph: link to the underlying VG.
pub const IRI_DEC_EXEMPLAR_OF: &str = "https://decision-cli.dev/ns#exemplarOf";

/// `dec:appliesToSafetyClass` — ExemplarGraph: which env safety class
/// this exemplar applies to.
pub const IRI_DEC_APPLIES_TO_SAFETY_CLASS: &str =
    "https://decision-cli.dev/ns#appliesToSafetyClass";

/// `dec:patternName` — ExemplarGraph: short slug.
pub const IRI_DEC_PATTERN_NAME: &str = "https://decision-cli.dev/ns#patternName";

/// `dec:basedOnApprovedResult` — ExemplarGraph: link to the
/// VerificationGraphResult that proves the exemplar.
pub const IRI_DEC_BASED_ON_APPROVED_RESULT: &str =
    "https://decision-cli.dev/ns#basedOnApprovedResult";

// `dec:supersededBy` IRI is exported from `vocab::feedback` (since
// FT-027 introduced it for feedback supersession). The catalog reuses
// that constant via `crate::vocab::IRI_DEC_SUPERSEDED_BY`.

// --- IRI prefixes -----------------------------------------------------------

/// IRI prefix for minted CapabilityReference IRIs.
pub const IRI_DEC_CR_PREFIX: &str = "https://decision-cli.dev/ns/cr/";
/// IRI prefix for minted OntologyDescription IRIs.
pub const IRI_DEC_OD_PREFIX: &str = "https://decision-cli.dev/ns/od/";
/// IRI prefix for minted ExemplarGraph IRIs.
pub const IRI_DEC_EX_PREFIX: &str = "https://decision-cli.dev/ns/ex/";

// --- Named graph IRIs -------------------------------------------------------

/// Named graph the catalog projections live in.
pub const IRI_DEC_GRAPH_CATALOG: &str = "https://decision-cli.dev/ns/graph/catalog";

#[must_use]
pub fn capability_reference_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_REFERENCE)
}

#[must_use]
pub fn ontology_description_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ONTOLOGY_DESCRIPTION)
}

#[must_use]
pub fn exemplar_graph_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EXEMPLAR_GRAPH)
}

#[must_use]
pub fn command_cr() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COMMAND_CR)
}

#[must_use]
pub fn capability_version_cr() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_VERSION_CR)
}

#[must_use]
pub fn capability_body() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CAPABILITY_BODY)
}

#[must_use]
pub fn namespace_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_NAMESPACE)
}

#[must_use]
pub fn prefix_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_PREFIX)
}

#[must_use]
pub fn ontology_version_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ONTOLOGY_VERSION)
}

#[must_use]
pub fn ontology_body() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ONTOLOGY_BODY)
}

#[must_use]
pub fn exemplar_of() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EXEMPLAR_OF)
}

#[must_use]
pub fn applies_to_safety_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_APPLIES_TO_SAFETY_CLASS)
}

#[must_use]
pub fn pattern_name() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_PATTERN_NAME)
}

#[must_use]
pub fn based_on_approved_result() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BASED_ON_APPROVED_RESULT)
}

#[must_use]
pub fn catalog_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_CATALOG)
}
