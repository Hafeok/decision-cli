//! `MatchReport` / `MatchKind` / `GraphSummary` value types (FT-046).
//!
//! Carried at the canonical IRI form so callers compare strings with
//! the same identifiers the SPARQL layer produces. Short-id callers
//! translate via the `feature_resolver` prefix helpers before invoking
//! [`super::best_matching_graphs`].

use oxigraph::model::NamedNode;

use crate::verify::coverage::{GraphId, TcId};
use dec_graph::ontology::verification_graph::ArtifactRef;

/// Stable IRI of a `dec:VerificationBench`
/// (`https://decision-cli.dev/ns/bench/ENV-…`).
pub type EnvId = String;

/// One graph in a match result. Lists the TCs *this graph alone*
/// covers from the feature's TC set, in feature-declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSummary {
    /// Stable IRI of the graph (`https://decision-cli.dev/ns/graph/VG-…`).
    pub id: GraphId,
    /// The graph's `dec:verifies` reference (polymorphic: feature or TC).
    pub verifies: ArtifactRef,
    /// Subset of the feature's TC list this graph covers on its own.
    pub covers: Vec<TcId>,
}

impl GraphSummary {
    /// Build a summary from raw IRIs (the form the SPARQL layer emits).
    #[must_use]
    pub(super) fn from_iris(graph_iri: &str, verifies_iri: &str, covers: Vec<TcId>) -> Self {
        Self {
            id: graph_iri.to_string(),
            verifies: ArtifactRef(NamedNode::new_unchecked(verifies_iri.to_string())),
            covers,
        }
    }
}

/// Coarse outcome shape. All four are valid (per FT-046 §Invariants):
/// the caller decides what to do with each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchKind {
    /// Exactly one graph covers every TC.
    CompleteSingle,
    /// A small set of graphs whose union covers every TC.
    CompleteMultiple,
    /// At least one TC remains uncovered after the greedy cover.
    Partial,
    /// No candidate graph in this env touches any of the feature's TCs.
    None,
}

/// Structural match answer for `(feature, env)` over the store.
///
/// `graphs` is ordered: greedy-cover insertion order for `Complete*`
/// and `Partial`; empty for `None`. `covered_by_match` and
/// `residual_uncovered` partition the feature's TC list in
/// declaration order (per FT-046 §Invariants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchReport {
    /// IRI of the feature the report describes.
    pub feature: TcId,
    /// IRI of the environment the report is scoped to.
    pub environment: EnvId,
    /// Coarse outcome shape (see [`MatchKind`]).
    pub kind: MatchKind,
    /// Chosen cover, in deterministic insertion order.
    pub graphs: Vec<GraphSummary>,
    /// TCs covered by the union of `graphs`.
    pub covered_by_match: Vec<TcId>,
    /// TCs no graph in this env covers.
    pub residual_uncovered: Vec<TcId>,
}
