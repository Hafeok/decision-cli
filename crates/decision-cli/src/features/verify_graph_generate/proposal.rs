//! `GraphProposal` data model (FT-049 / ADR-030).
//!
//! Mirror of the Python worker's pydantic shape (`workers/verify-graph-author/
//! src/verify_graph_author/output.py`). Round-trips byte-equal through JSON
//! so the harness can verify the worker's stdout shape verbatim.
//!
//! Three discriminated variants share a `bundle_hash` echo field used as
//! the proposal token in the MCP two-call protocol.

use serde::{Deserialize, Serialize};

/// The kind discriminator on a `GraphProposal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProposalKind {
    /// An existing graph already covers the feature's TCs in the env.
    Match,
    /// Author a fresh graph in the target env.
    New,
    /// Worker cannot honestly produce a covering graph.
    Gap,
}

/// `match` payload — the worker picked an existing graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchProposal {
    /// IRI / identifier of the matched existing graph.
    pub graph_id: String,
    /// One-line justification (worker prose).
    pub rationale: String,
}

/// One step in a `new` proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedStep {
    /// One of the six FT-036 seed kinds.
    pub step_type: String,
    /// Per-kind payload validated client-side via `verify_step_add::fields`.
    #[serde(default)]
    pub fields: serde_json::Map<String, serde_json::Value>,
    /// TC ids this step provides evidence for.
    #[serde(default)]
    pub provides_evidence_for: Vec<String>,
}

/// `new` payload — propose a fresh graph in the target env.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewProposal {
    /// Target environment id / IRI.
    pub environment: String,
    /// Ordered list of proposed steps.
    pub steps: Vec<ProposedStep>,
    /// Why this step sequence covers the feature's TCs.
    pub rationale: String,
}

/// `gap` payload — worker cannot produce a covering graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapProposal {
    /// TC ids the worker cannot cover.
    pub uncovered_tcs: Vec<String>,
    /// Why the step vocabulary or environment is insufficient.
    pub reason: String,
}

/// The structured artifact the verify-graph-author worker returns.
///
/// Exactly one of `match`/`new`/`gap` is populated; `kind` is the
/// discriminator. `bundle_hash` echoes the input hash so the harness can
/// detect protocol violations (FT-048 §Error 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphProposal {
    /// Discriminator on the populated payload.
    pub kind: ProposalKind,
    /// Echo of the input bundle's SHA-256 (hex) — the integrity check.
    pub bundle_hash: String,
    /// Populated iff `kind == Match`.
    #[serde(rename = "match", default, skip_serializing_if = "Option::is_none")]
    pub match_payload: Option<MatchProposal>,
    /// Populated iff `kind == New`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<NewProposal>,
    /// Populated iff `kind == Gap`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<GapProposal>,
}

impl GraphProposal {
    /// Construct a `Match` variant.
    #[must_use]
    pub fn new_match(bundle_hash: impl Into<String>, payload: MatchProposal) -> Self {
        Self {
            kind: ProposalKind::Match,
            bundle_hash: bundle_hash.into(),
            match_payload: Some(payload),
            new: None,
            gap: None,
        }
    }

    /// Construct a `New` variant.
    #[must_use]
    pub fn new_new(bundle_hash: impl Into<String>, payload: NewProposal) -> Self {
        Self {
            kind: ProposalKind::New,
            bundle_hash: bundle_hash.into(),
            match_payload: None,
            new: Some(payload),
            gap: None,
        }
    }

    /// Construct a `Gap` variant.
    #[must_use]
    pub fn new_gap(bundle_hash: impl Into<String>, payload: GapProposal) -> Self {
        Self {
            kind: ProposalKind::Gap,
            bundle_hash: bundle_hash.into(),
            match_payload: None,
            new: None,
            gap: Some(payload),
        }
    }
}

/// Coverage roll-up included in the response payload alongside the
/// proposal (FT-049 §Outputs `coverage_preview` and `coverage_report`).
///
/// Lists are the short ids (`TC-NNN`) and short graph ids (`VG-NNN`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageReportSummary {
    /// TCs the matcher considers covered for the (feature, env) query.
    #[serde(default)]
    pub covered: Vec<String>,
    /// TCs still uncovered after the matcher (or after the new graph
    /// would be persisted, in `accept` responses).
    #[serde(default)]
    pub uncovered: Vec<String>,
    /// Graph ids the report consulted.
    #[serde(default)]
    pub considered: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_round_trips_through_json() {
        let p = GraphProposal::new_match(
            "abcdef0123456789",
            MatchProposal {
                graph_id: "VG-007".to_string(),
                rationale: "covers all TCs".to_string(),
            },
        );
        let v = serde_json::to_value(&p).expect("ser");
        let back: GraphProposal = serde_json::from_value(v).expect("de");
        assert_eq!(p, back);
    }

    #[test]
    fn new_round_trips_through_json() {
        let p = GraphProposal::new_new(
            "abcdef0123456789",
            NewProposal {
                environment: "ENV-1".to_string(),
                steps: vec![ProposedStep {
                    step_type: "shell-command".to_string(),
                    fields: serde_json::json!({"command": "ls"})
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                    provides_evidence_for: vec!["TC-A".to_string()],
                }],
                rationale: "step covers TC-A".to_string(),
            },
        );
        let v = serde_json::to_value(&p).expect("ser");
        let back: GraphProposal = serde_json::from_value(v).expect("de");
        assert_eq!(p, back);
    }
}
