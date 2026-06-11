//! Request / response / error envelopes for `core::verify::runner` (FT-098).
//!
//! The runner is the single chokepoint that turns a `dec:VerificationGraph`
//! into a `dec:VerificationGraphResult`. CLI (FT-099) and subscription
//! (FT-100) callers construct a [`RunGraphRequest`] and route it through
//! [`super::run_graph`].

use std::collections::HashMap;
use std::path::PathBuf;

use oxigraph::model::NamedNode;
use thiserror::Error;

use dec_graph::ontology::verdict::Verdict;
use dec_graph::ontology::verification_result::{StepOutcome, VerificationGraphResult};

/// What triggered the runner invocation. PROV-O `wasInformedBy` chains
/// are built from this tag at the call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerKind {
    /// `dec verify graph run` (slice-2.5 CLI entry).
    Manual,
    /// `graph_accepted_dispatch` subscription (FT-100).
    GraphAccepted,
    /// `code_change_committed_dispatch` subscription (FT-100).
    CodeChangeCommitted {
        /// IRI of the `dec:CodeChange` that triggered the run.
        code_change: NamedNode,
    },
    /// `dec verify feature` aggregate roll-up entry.
    Aggregate {
        /// Feature IRI rolling up the aggregate verdict.
        feature: NamedNode,
    },
}

impl TriggerKind {
    /// Short stable tag used in PROV-O event payloads.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::GraphAccepted => "graph-accepted",
            Self::CodeChangeCommitted { .. } => "code-change-committed",
            Self::Aggregate { .. } => "aggregate",
        }
    }
}

/// Input bundle the runner receives. The graph + env are loaded by the
/// runner from `.dec/verify/{graph,env}/<id>.ttl` per FT-098 §Inputs.
#[derive(Debug, Clone)]
pub struct RunGraphRequest {
    /// IRI of the `dec:VerificationGraph` to execute.
    pub graph: NamedNode,
    /// Why the runner was invoked.
    pub triggered_by: TriggerKind,
    /// Pre-seeded capture bindings (e.g. `${health_url}` for code-change
    /// triggered runs). Empty for manual invocations.
    pub capture_bindings: HashMap<String, String>,
    /// PROV-O activity opened by the caller — every persisted artifact
    /// links back to this via `prov:wasGeneratedBy`.
    pub run_activity: NamedNode,
    /// `.dec/`-bootstrapped working tree the runner operates against.
    pub workdir: PathBuf,
}

/// Per-step outcome mirror returned to the caller — the persisted result
/// remains the single source of truth, but having the trace pattern in
/// memory lets CLI/MCP rendering avoid a re-read.
#[derive(Debug, Clone)]
pub struct StepOutcomeSummary {
    /// Two-tier outcome per ADR-013 / ADR-028.
    pub outcome: StepOutcome,
    /// IRI of the corresponding `dec:VerificationStep` in the parent graph.
    pub step_id: NamedNode,
    /// IRI of the corresponding `dec:VerificationStepTrace` on the result.
    pub trace_id: String,
}

/// The runner's return envelope.
#[derive(Debug, Clone)]
pub struct RunGraphResponse {
    /// IRI of the persisted `dec:VerificationGraphResult`.
    pub result: NamedNode,
    /// Per-graph verdict per FT-097 / ADR-028.
    pub verdict: Verdict,
    /// Step traces in graph order. Length always equals the parent
    /// graph's step count, except for Phase-1 aborts (empty).
    pub step_outcomes: Vec<StepOutcomeSummary>,
    /// IRIs of any `dec:Feedback` artifacts emitted for failing
    /// evidence-bearing steps (FT-026 / ADR-022). Empty when the run
    /// produced no failures.
    pub emitted_feedback: Vec<NamedNode>,
    /// Full persisted `VerificationGraphResult` artifact (FT-097). Used
    /// by FT-099's `dec verify feature` aggregator to feed
    /// `aggregate_verdict` without a separate store round-trip.
    pub result_artifact: VerificationGraphResult,
}

/// Runner failure surface. Per FT-098 §Error handling, per-step failures
/// do **not** surface as errors — they are encoded as outcomes on the
/// trace. Only failures preventing a result artifact from being produced
/// at all bubble up here.
#[derive(Debug, Error)]
pub enum RunnerError {
    /// The graph or env IRI does not resolve in the on-disk catalog.
    #[error("artifact not found: {kind} <{id}>")]
    ArtifactNotFound {
        /// Artifact kind (`VerificationGraph`, `VerificationBench`).
        kind: String,
        /// The id or IRI we tried to load.
        id: String,
    },

    /// Phase-1 defensive op-gate fired. A `VerificationGraphResult` with
    /// `verdict = rejected` and empty `stepTraces` IS persisted before
    /// the runner returns this error.
    #[error("safety violation: step <{step}> requires op <{op}> not in env.allowedOps")]
    SafetyViolation {
        /// IRI of the offending step.
        step: String,
        /// The op the step required but the env did not allow.
        op: String,
    },

    /// `env.dec:setup` returned a non-zero exit. A result with
    /// `verdict = amendment-required` IS persisted (best-effort).
    #[error("env setup script failed: exit {exit_code}: {stderr_excerpt}")]
    EnvSetupFailed {
        /// Setup script exit code.
        exit_code: i32,
        /// Excerpt of stderr (cap 4 KiB).
        stderr_excerpt: String,
    },

    /// `StreamWriter::commit` rejected the result artifact (SHACL,
    /// content-hash collision, transactional error). The caller is
    /// responsible for closing the activity with a failure note.
    #[error("result persistence failed: {source}")]
    ResultWriteFailed {
        /// Underlying error.
        #[source]
        source: anyhow::Error,
    },

    /// Internal / I/O failure outside any of the above categories.
    #[error("internal runner error: {detail}")]
    Internal {
        /// Diagnostic message.
        detail: String,
    },
}
