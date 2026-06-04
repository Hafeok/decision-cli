//! `Action` — what the planner tells the driver to do next.

use serde::{Deserialize, Serialize};

/// Closed enumeration of next-step actions. The driver dispatches each
/// non-terminal variant by calling into the matching feature handler;
/// terminal variants end the loop.
///
/// New variants would only be added when a fundamentally new worker
/// role appears. Each existing planner returns a subset of these — for
/// instance the FT+Ship planner never returns `Action::Done` with a
/// `feature_id`, only as a marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Action {
    /// Terminal — goal reached. Driver returns `DriveOutcome::Reached`.
    Done,
    /// Run `dec verify feature <feature_id>` (FT-099).
    DispatchVerifier {
        /// Feature short id.
        feature_id: String,
        /// Env short id. Some planners may default this from the
        /// feature's last verify run; the driver passes it through.
        env_id: String,
    },
    /// Run `dec implement <feature_id>` (FT-013 + FT-108).
    DispatchImplementer {
        /// Feature short id.
        feature_id: String,
    },
    /// Run `dec verify graph generate <feature_id> --environment <env_id>
    /// --accept` (FT-107).
    DispatchVerifyGraphAuthor {
        /// Feature short id.
        feature_id: String,
        /// Env short id.
        env_id: String,
    },
    /// Dispatch tc-author worker to author missing TCs (FT-126, FT-131).
    DispatchTcAuthor {
        /// Feature short id.
        feature_id: String,
        /// Target count of TCs to reach (ADR-072 floor).
        target_count: usize,
    },
    /// Dispatch tc-quality judge on a pending TC proposal (FT-127, FT-131).
    DispatchTcQuality {
        /// Feature short id.
        feature_id: String,
        /// IRI of the pending TC proposal artifact.
        tc_proposal_iri: String,
    },
    /// Dispatch vg-quality judge on a pending VG proposal (FT-128, FT-131).
    DispatchVgQuality {
        /// Feature short id.
        feature_id: String,
        /// IRI of the pending graph proposal artifact.
        graph_proposal_iri: String,
    },
    /// Dispatch spec-author worker to author missing spec sections (FT-129, FT-131, Slice B).
    DispatchSpecAuthor {
        /// Feature short id.
        feature_id: String,
    },
    /// Dispatch adr-author worker to close a preflight gap (FT-130, FT-131, Slice B).
    DispatchAdrAuthor {
        /// Feature short id.
        feature_id: String,
        /// Preflight gap detail (unacknowledged ADR or domain).
        preflight_gap: String,
    },
    /// Escalation: the verify-graph-author can't fix the open defects
    /// (typically because the underlying commands the graph invokes
    /// fail with general errors — exit 1 — that signal a missing or
    /// broken *implementation*, not a graph-design issue). The
    /// executor re-routes the feature's open verifier-targeted
    /// defects to the implementer via the ADR-024 lifecycle and then
    /// dispatches the implementer with the rerouted feedback in its
    /// bundle.
    EscalateVgaToImplementer {
        /// Feature short id.
        feature_id: String,
    },
    /// Reverse escalation: the implementer can't address the open
    /// defects (typically because the failing step in the graph
    /// exercises something out-of-scope for the feature spec — a
    /// command in a different repo, a transient environment issue, or
    /// a test that was authored against the wrong artifact). The
    /// executor re-routes the feature's open implementer-targeted
    /// defects to the verifier via the ADR-024 lifecycle and then
    /// dispatches the verify-graph-author to re-author the test.
    EscalateImplementerToVga {
        /// Feature short id.
        feature_id: String,
        /// Env short id.
        env_id: String,
    },
    /// Terminal — no path forward. Driver returns `DriveError::Stuck`.
    /// The `reason` is rendered verbatim so the operator sees the
    /// planner's diagnosis.
    Stuck {
        /// Renderable reason ("worker not converging", "TC body
        /// missing", etc.).
        reason: String,
    },
}

impl Action {
    /// True iff the variant terminates the driver loop.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Stuck { .. })
    }

    /// Short tag for history rendering.
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::DispatchVerifier { .. } => "dispatch:verifier",
            Self::DispatchImplementer { .. } => "dispatch:implementer",
            Self::DispatchVerifyGraphAuthor { .. } => "dispatch:verify-graph-author",
            Self::DispatchTcAuthor { .. } => "dispatch:tc-author",
            Self::DispatchTcQuality { .. } => "dispatch:tc-quality",
            Self::DispatchVgQuality { .. } => "dispatch:vg-quality",
            Self::DispatchSpecAuthor { .. } => "dispatch:spec-author",
            Self::DispatchAdrAuthor { .. } => "dispatch:adr-author",
            Self::EscalateVgaToImplementer { .. } => "escalate:vga-to-implementer",
            Self::EscalateImplementerToVga { .. } => "escalate:implementer-to-vga",
            Self::Stuck { .. } => "stuck",
        }
    }
}
