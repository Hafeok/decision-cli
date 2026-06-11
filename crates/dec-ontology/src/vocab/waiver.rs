//! FT-047 / ADR-031 — `dec:CoverageWaiver` vocabulary.
//!
//! Constants for the artifact the chain-integrity gate writes when a
//! caller invokes a feature-targeted dispatch with `--waive-coverage`.
//! Mirrors the verification-graph and verification-env vocab modules in
//! shape; split out of `core::vocab::mod` to keep per-file line counts
//! within the ADR-013 ceiling.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

/// Class IRI for `dec:CoverageWaiver` (ADR-031).
pub const IRI_DEC_COVERAGE_WAIVER: &str = "https://decision-cli.dev/ns#CoverageWaiver";

/// `dec:waiverFor` predicate — `CoverageWaiver` → feature IRI.
pub const IRI_DEC_WAIVER_FOR: &str = "https://decision-cli.dev/ns#waiverFor";

/// `dec:waiverReason` predicate — free-form prose, SHACL min 16 chars.
pub const IRI_DEC_WAIVER_REASON: &str = "https://decision-cli.dev/ns#waiverReason";

/// `dec:uncoveredAtWaive` predicate — TC IRI snapshot at gate-firing time.
pub const IRI_DEC_UNCOVERED_AT_WAIVE: &str = "https://decision-cli.dev/ns#uncoveredAtWaive";

/// `prov:wasAttributedTo` — PROV-O attribution to the dispatching agent.
pub const IRI_PROV_WAS_ATTRIBUTED_TO: &str = "http://www.w3.org/ns/prov#wasAttributedTo";

/// `dcterms:created` — RFC3339 timestamp of waiver minting.
pub const IRI_DCTERMS_CREATED: &str = "http://purl.org/dc/terms/created";

/// `prov:used` — re-export of the predicate the dispatch activity uses to
/// reference the waiver in its PROV-O chain (mirrors implementer vocab).
pub const IRI_PROV_USED: &str = "http://www.w3.org/ns/prov#used";

/// Named graph the waiver projections live in.
pub const IRI_DEC_GRAPH_WAIVERS: &str = "https://decision-cli.dev/ns/graph/waivers";

/// IRI prefix for minted waiver IRIs (`https://decision-cli.dev/ns/waiver/<id>`).
pub const IRI_DEC_WAIVER_PREFIX: &str = "https://decision-cli.dev/ns/waiver/";

/// Minimum length, in non-whitespace characters, of a waiver reason
/// (ADR-031 / FT-047 §Error handling).
pub const WAIVER_REASON_MIN_LEN: usize = 16;

#[must_use]
pub fn coverage_waiver_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_COVERAGE_WAIVER)
}

#[must_use]
pub fn waiver_for() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_WAIVER_FOR)
}

#[must_use]
pub fn waiver_reason() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_WAIVER_REASON)
}

#[must_use]
pub fn uncovered_at_waive() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_UNCOVERED_AT_WAIVE)
}

#[must_use]
pub fn was_attributed_to() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_WAS_ATTRIBUTED_TO)
}

#[must_use]
pub fn dcterms_created() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DCTERMS_CREATED)
}

#[must_use]
pub fn prov_used() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_PROV_USED)
}

#[must_use]
pub fn waivers_named_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_WAIVERS)
}
