//! FT-097 / ADR-028 — verification-result artifact vocabulary.
//!
//! Split out of `core::vocab` to keep per-file size within the ADR-013
//! 400-line ceiling. Re-exported from the parent module so callers
//! continue to import from `decision_cli::vocab`.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

// --- Class IRIs --------------------------------------------------------------

/// Class IRI for `dec:VerificationGraphResult` (ADR-028 §VerificationGraphResult).
pub const IRI_DEC_VERIFICATION_GRAPH_RESULT: &str =
    "https://decision-cli.dev/ns#VerificationGraphResult";

/// Class IRI for `dec:VerificationStepTrace`.
pub const IRI_DEC_VERIFICATION_STEP_TRACE: &str =
    "https://decision-cli.dev/ns#VerificationStepTrace";

/// Class IRI for `dec:EvidenceProjection` (derived blank-node).
pub const IRI_DEC_EVIDENCE_PROJECTION: &str = "https://decision-cli.dev/ns#EvidenceProjection";

// --- Result-level predicates ------------------------------------------------

/// `dec:resultOf` predicate — result → VerificationGraph.
pub const IRI_DEC_RESULT_OF: &str = "https://decision-cli.dev/ns#resultOf";

/// `dec:ranInEnvironment` predicate — result → VerificationEnvironment.
pub const IRI_DEC_RAN_IN_ENVIRONMENT: &str = "https://decision-cli.dev/ns#ranInEnvironment";

/// `dec:stepTraces` predicate — result → rdf:List of step trace IRIs.
pub const IRI_DEC_STEP_TRACES: &str = "https://decision-cli.dev/ns#stepTraces";

/// `dec:evidenceFor` predicate — result → EvidenceProjection blank node.
pub const IRI_DEC_EVIDENCE_FOR: &str = "https://decision-cli.dev/ns#evidenceFor";

/// `dec:startedAt` predicate — xsd:dateTime.
pub const IRI_DEC_STARTED_AT: &str = "https://decision-cli.dev/ns#startedAt";

/// `dec:endedAt` predicate — xsd:dateTime.
pub const IRI_DEC_ENDED_AT: &str = "https://decision-cli.dev/ns#endedAt";

// --- Step-trace-level predicates --------------------------------------------

/// `dec:tracesStep` predicate — StepTrace → VerificationStep.
pub const IRI_DEC_TRACES_STEP: &str = "https://decision-cli.dev/ns#tracesStep";

/// `dec:outcome` predicate — StepTrace → outcome literal.
pub const IRI_DEC_OUTCOME: &str = "https://decision-cli.dev/ns#outcome";

/// `dec:exitCode` predicate — StepTrace → xsd:integer.
pub const IRI_DEC_EXIT_CODE: &str = "https://decision-cli.dev/ns#exitCode";

/// `dec:stdoutExcerpt` predicate — StepTrace → string (cap 4 KiB).
pub const IRI_DEC_STDOUT_EXCERPT: &str = "https://decision-cli.dev/ns#stdoutExcerpt";

/// `dec:stderrExcerpt` predicate — StepTrace → string (cap 4 KiB).
pub const IRI_DEC_STDERR_EXCERPT: &str = "https://decision-cli.dev/ns#stderrExcerpt";

/// `dec:errorMessage` predicate — StepTrace → string (when outcome != pass).
pub const IRI_DEC_ERROR_MESSAGE: &str = "https://decision-cli.dev/ns#errorMessage";

// --- EvidenceProjection predicates ------------------------------------------

/// `dec:tc` predicate — EvidenceProjection → TC IRI.
pub const IRI_DEC_TC: &str = "https://decision-cli.dev/ns#tc";

/// `dec:fromStep` already exists in verify_graph for capture; we reuse the
/// IRI but the EvidenceProjection projects a step-trace IRI.

// --- Named graph + IRI prefixes ---------------------------------------------

/// Named graph holding verification-graph-result projections.
pub const IRI_DEC_GRAPH_VERIFY_RESULT: &str = "https://decision-cli.dev/ns/graph/verify-result";

/// IRI prefix for minted result IRIs (`…/ns/result/VGR-NNN`).
pub const IRI_DEC_VERIFY_RESULT_PREFIX: &str = "https://decision-cli.dev/ns/result/";

// --- Outcome literal tokens -------------------------------------------------

/// Step trace outcome: `pass`.
pub const OUTCOME_PASS: &str = "pass";
/// Step trace outcome: `fail`.
pub const OUTCOME_FAIL: &str = "fail";
/// Step trace outcome: `unrunnable`.
pub const OUTCOME_UNRUNNABLE: &str = "unrunnable";

/// All allowed outcome tokens in declaration order.
pub const OUTCOME_TOKENS: &[&str] = &[OUTCOME_PASS, OUTCOME_FAIL, OUTCOME_UNRUNNABLE];

/// Excerpt cap (per FT-097 §Invariants — 4 KiB).
pub const EXCERPT_CAP_BYTES: usize = 4096;

// --- NamedNodeRef helpers ---------------------------------------------------

#[must_use]
pub fn verification_graph_result_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_GRAPH_RESULT)
}

#[must_use]
pub fn verification_step_trace_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_STEP_TRACE)
}

#[must_use]
pub fn evidence_projection_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EVIDENCE_PROJECTION)
}

#[must_use]
pub fn result_of_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_RESULT_OF)
}

#[must_use]
pub fn ran_in_environment_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_RAN_IN_ENVIRONMENT)
}

#[must_use]
pub fn step_traces_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STEP_TRACES)
}

#[must_use]
pub fn evidence_for_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EVIDENCE_FOR)
}

#[must_use]
pub fn started_at_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STARTED_AT)
}

#[must_use]
pub fn ended_at_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ENDED_AT)
}

#[must_use]
pub fn traces_step_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TRACES_STEP)
}

#[must_use]
pub fn outcome_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_OUTCOME)
}

#[must_use]
pub fn exit_code_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_EXIT_CODE)
}

#[must_use]
pub fn stdout_excerpt_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STDOUT_EXCERPT)
}

#[must_use]
pub fn stderr_excerpt_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_STDERR_EXCERPT)
}

#[must_use]
pub fn error_message_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_ERROR_MESSAGE)
}

#[must_use]
pub fn tc_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TC)
}

#[must_use]
pub fn verify_result_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_VERIFY_RESULT)
}
