//! Goal vocabulary — the set of value-actions `dec drive` understands.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Closed enumeration of goals. New goals append here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Goal {
    /// Drive a feature all the way to "shipped" (verify passes,
    /// status=complete).
    Ship,
    /// Drive a feature through one verify cycle (no implicit
    /// re-implementation).
    Verify,
    /// Drive a feature through the Definition-of-Ready gate
    /// (FT-119). Authors a `VerificationGraph` via the
    /// verify-graph-author worker when one is missing; returns
    /// `Stuck` on every gap that requires human-authored content
    /// (spec body, preflight ADR ack, missing TCs).
    DefReady,
    /// Drive an ADR through review to accepted status. Placeholder for
    /// the future ADR+Accept planner.
    Accept,
    /// Drive a TC to "covered" (has a passing graph). Placeholder.
    Cover,
    /// Drive a VG to an approved VGR. Placeholder.
    Approve,
}

impl Goal {
    /// Stable wire string for CLI parsing and JSON rendering.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ship => "ship",
            Self::Verify => "verify",
            Self::DefReady => "def-ready",
            Self::Accept => "accept",
            Self::Cover => "cover",
            Self::Approve => "approve",
        }
    }

    /// Parse from the CLI wire string. Unknown values return `Err`.
    pub fn parse(s: &str) -> Result<Self, GoalParseError> {
        match s {
            "ship" => Ok(Self::Ship),
            "verify" => Ok(Self::Verify),
            "def-ready" => Ok(Self::DefReady),
            "accept" => Ok(Self::Accept),
            "cover" => Ok(Self::Cover),
            "approve" => Ok(Self::Approve),
            _ => Err(GoalParseError::Unknown {
                input: s.to_string(),
            }),
        }
    }
}

impl fmt::Display for Goal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Goal-parse error variants.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GoalParseError {
    /// String didn't match any known goal.
    #[error("unknown goal {input:?}; supported: ship, verify, def-ready, accept, cover, approve")]
    Unknown {
        /// The raw input string.
        input: String,
    },
}
