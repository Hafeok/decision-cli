//! Data model for drive rounds (FT-113).

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Round {
    pub round_index: u32,
    pub started_at: DateTime<Utc>,
    pub elapsed_since_round_zero: Duration,
    pub state: RoundState,
    pub dispatch: Dispatch,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoundState {
    pub verdict: String, // e.g. "NeverRun", "Rejected", "Approved"
    pub impl_open: usize,
    pub vga_open: usize,
    pub graph_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dispatch {
    pub role: String,           // e.g. "verify-graph-author", "implementer", "verifier"
    pub session_iri: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Outcome {
    #[serde(rename = "vga-produced")]
    VgaProduced {
        graph_id: String,
        steps: usize,
        pass: usize,
        fail: usize,
    },
    #[serde(rename = "implementer-commit")]
    ImplementerCommit {
        sha: String,
        addressed: usize,
        remaining: usize,
    },
    #[serde(rename = "verifier-results")]
    VerifierResults {
        pass: usize,
        fail: usize,
    },
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "stuck")]
    Stuck { reason: String },
    #[serde(rename = "partial")]
    Partial { reason: String },
}
