//! `CoverageReport` / `CoverageHit` value types (FT-045 / ADR-030).
//!
//! All ids are carried as full IRIs so callers can compare with the
//! same strings the SPARQL layer produces. Short-id callers (e.g.
//! `FT-001`, `VG-001`, `TC-068`) translate via
//! [`super::feature_resolver`]'s prefix helpers before invocation.

/// Stable IRI of a feature artifact (`https://decision-cli.dev/ns/feature/FT-…`).
pub type FeatureId = String;
/// Stable IRI of a test criterion (`https://decision-cli.dev/ns/tc/TC-…`).
pub type TcId = String;
/// Stable IRI of a `dec:VerificationGraph` (`https://decision-cli.dev/ns/graph/VG-…`).
pub type GraphId = String;
/// Stable IRI of a `dec:VerificationStep`.
pub type StepId = String;

/// A single covering step for a particular TC. Aggregated into
/// [`CoverageReport::covered`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoverageHit {
    /// IRI of the TC the hit covers.
    pub tc: TcId,
    /// IRI of the graph carrying the covering step.
    pub graph: GraphId,
    /// IRI of the covering step (carries `dec:providesEvidenceFor ?tc`).
    pub step: StepId,
}

/// Structural coverage answer for a feature against a set of graphs.
///
/// `covered` lists every covering step, `uncovered` lists TCs with zero
/// covering steps in `considered`. `all_tcs` carries the feature's full
/// TC list in declaration order; `uncovered` preserves that order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageReport {
    /// IRI of the feature the report describes.
    pub feature: FeatureId,
    /// Every TC the feature lists (declaration order).
    pub all_tcs: Vec<TcId>,
    /// One entry per (tc, graph, step) covering hit.
    pub covered: Vec<CoverageHit>,
    /// TCs with zero covering steps across `considered`.
    pub uncovered: Vec<TcId>,
    /// Graph IRIs actually consulted by the query (after candidate
    /// resolution / discovery).
    pub considered: Vec<GraphId>,
}
