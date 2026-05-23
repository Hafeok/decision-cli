//! Typed payloads for the escalation loop (FT-062).

use oxigraph::model::NamedNode;
use thiserror::Error;

use crate::core::bundle::Stakes;
use crate::core::dispatch::capability_resolver::{ResolvedCapability, ResolverError};
use crate::core::feedback::FeedbackClass;
use crate::core::ontology::capability::CapabilityReadError;
use crate::core::ontology::role_binding::RoleBindingReadError;
use crate::core::ontology::verdict::Verdict;

/// Identifier for a dispatched session.
pub type SessionId = NamedNode;

/// Outcome of the worker's optional audit phase.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditOutcome {
    /// True if the audit passed; false on failure.
    pub passes: bool,
}

/// Structured worker result — the bits the dispatcher reads at
/// signal-collection time.
///
/// `code-writer` and friends produce a `CodeChange` (no verdict),
/// `verifier` produces a `VerificationVerdict`. Only the verdict and
/// confidence are read by the dispatcher; the full artifact data is
/// recorded on the session record's PROV-O lineage.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerResult {
    /// Verifier-style result with verdict + confidence.
    Verdict {
        /// Verdict kind from ADR-018.
        kind: Verdict,
        /// Confidence in `[0.0, 1.0]` — None if not produced.
        confidence: Option<f32>,
    },
    /// Implementer-style result: a CodeChange (or any non-verdict
    /// artifact). `applied = false` is treated as a failed worker
    /// attempt the dispatcher may still escalate from.
    CodeChange {
        /// Whether the change was applied successfully.
        applied: bool,
    },
    /// Generic worker error (subprocess crash, schema violation).
    Failed,
}

impl WorkerResult {
    /// Confidence value, when applicable.
    #[must_use]
    pub fn confidence(&self) -> Option<f32> {
        match self {
            Self::Verdict { confidence, .. } => *confidence,
            _ => None,
        }
    }

    /// Verdict kind, when applicable.
    #[must_use]
    pub fn verdict(&self) -> Option<Verdict> {
        match self {
            Self::Verdict { kind, .. } => Some(*kind),
            _ => None,
        }
    }
}

/// One emitted feedback artifact's projection — class + severity. The
/// dispatcher only reads these fields per FT-062 signal-collection.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackArtifact {
    /// Controlled-vocabulary class per ADR-023.
    pub class: FeedbackClass,
    /// True iff severity is `"critical"` — what
    /// `feedback_unimplementable_critical` reads.
    pub critical: bool,
}

/// One attempt within an escalation chain — the result of a single
/// `(role, capability, bundle)` invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchAttempt {
    /// Session IRI minted for this attempt.
    pub session_id: SessionId,
    /// Capability that ran the attempt.
    pub capability: ResolvedCapability,
    /// Worker's structured result.
    pub result: WorkerResult,
    /// Feedback artifacts the worker emitted in this attempt.
    pub feedback: Vec<FeedbackArtifact>,
    /// Audit outcome, if an audit ran.
    pub audit_outcome: Option<AuditOutcome>,
}

/// Telemetry counters the dispatcher records on each attempt for
/// observability (`dec session show`, fitness functions).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttemptTokens {
    /// Base input tokens (always billed at `cost_input_per_m`).
    pub input_base: u64,
    /// Cache-write input tokens (Anthropic only).
    pub input_cache_write: u64,
    /// Cache-hit input tokens (Anthropic only).
    pub input_cache_hit: u64,
    /// Output tokens emitted by the model.
    pub output: u64,
}

/// All signals the dispatcher considers at escalation-decision time.
///
/// Per ADR-034 the trigger vocabulary is closed; this struct is the
/// closed-vocabulary domain over which trigger predicates evaluate.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalSet {
    /// Bundle stakes (from FT-056).
    pub stakes: Stakes,
    /// Audit outcome's `passes` flag — `Some(true/false)` when an
    /// audit ran, `None` otherwise.
    pub audit_pass: Option<bool>,
    /// Distinct feedback classes the attempt produced.
    pub feedback_classes: Vec<FeedbackClass>,
    /// True iff at least one emitted feedback was critical.
    pub feedback_critical: bool,
    /// Worker's confidence if the result was a verdict.
    pub confidence: Option<f32>,
    /// Number of prior attempts including this one — 1-indexed so
    /// `prior_attempts_ge_3` fires on the 3rd attempt onwards.
    pub prior_attempts: u32,
    /// Verdict kind if applicable.
    pub verdict: Option<Verdict>,
}

/// Errors raised by the FT-062 dispatcher loop.
#[derive(Debug, Error)]
pub enum EscalationError {
    /// Initial capability resolution failed (no binding, EOL, etc.).
    #[error("default capability resolution failed: {0}")]
    InitialResolution(#[from] ResolverError),

    /// Resolving the escalated capability failed mid-loop (catalog
    /// superseded the step target into EOL).
    #[error("escalated capability <{iri}> resolution failed: {detail}")]
    CapabilityResolutionFailed {
        /// Capability IRI the dispatcher tried to resolve.
        iri: String,
        /// Underlying diagnostic.
        detail: String,
    },

    /// The escalated step's `step_capability` IRI does not exist in
    /// the catalog (catalog corruption — should not happen if SHACL
    /// passed).
    #[error("escalated capability <{iri}> not found in catalog")]
    UnknownCapability {
        /// Capability IRI the dispatcher tried to look up.
        iri: String,
    },

    /// No active `dec:RoleBinding` for the requested role.
    #[error("no active role binding for {role_id:?}")]
    NoActiveBinding {
        /// Role being dispatched.
        role_id: String,
    },

    /// Generic graph-write failure during session linkage.
    #[error("graph write failure: {0}")]
    GraphWrite(String),

    /// Worker subprocess returned an error the dispatcher could not
    /// translate to a structured `WorkerResult`.
    #[error("worker runtime error: {0}")]
    WorkerRuntime(String),
}

impl From<RoleBindingReadError> for EscalationError {
    fn from(value: RoleBindingReadError) -> Self {
        Self::GraphWrite(value.to_string())
    }
}

impl From<CapabilityReadError> for EscalationError {
    fn from(value: CapabilityReadError) -> Self {
        Self::GraphWrite(value.to_string())
    }
}
