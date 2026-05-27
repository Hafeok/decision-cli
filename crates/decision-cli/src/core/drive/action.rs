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
            Self::EscalateVgaToImplementer { .. } => "escalate:vga-to-implementer",
            Self::Stuck { .. } => "stuck",
        }
    }
}
