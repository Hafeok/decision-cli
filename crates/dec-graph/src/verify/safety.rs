//! Static safety enforcement for verification graphs (FT-037 / ADR-028).
//!
//! For every step of a graph this module enforces
//! `step.requiredOps ⊆ env.allowedOps`. The check is pure given the
//! graph (or step) and the env it receives; no I/O, no global state.
//!
//! The functions exported here are the single chokepoint per ADR-028
//! §Safety gating — `StreamWriter` invokes them on every relevant
//! commit, and slice-3 dispatch will reuse the same surface for the
//! pre-dispatch defensive check.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ontology::verification_bench::VerificationBench;
use crate::ontology::verification_graph::{
    StepFields, StepIri, StepKind, VerificationGraph, VerificationStep,
};

/// The controlled vocabulary of operation tokens per ADR-028. Step
/// kinds declare a `requiredOps` subset of this set; envs declare
/// their `allowedOps` likewise. Any token outside this set in either
/// position surfaces [`SafetyError::UnknownOp`] rather than passing
/// silently.
const KNOWN_OPS: &[&str] = &[
    "shell",
    "filesystem",
    "sparql-local",
    "sparql-http",
    "http",
    "http-readonly",
    "http-mutating",
];

/// Either side of an op declaration — used by [`SafetyError::UnknownOp`]
/// so callers can attribute a malformed token to the right artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpSource {
    /// The token appears in `step.requiredOps` (declared by a step kind).
    Step,
    /// The token appears in `env.allowedOps` (declared by the environment).
    Env,
}

impl OpSource {
    /// Stable string form for diagnostics.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Env => "env",
        }
    }
}

impl std::fmt::Display for OpSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// `OpSource` is a tag, not a wrapped error — but thiserror's `#[source]`
// auto-decoration (triggered by the field name `source`) requires it to
// implement `std::error::Error`. The default `Error::source` returns
// `None`, which is correct for a tag.
impl std::error::Error for OpSource {}

/// A single safety violation carrying full diagnostic context per
/// FT-037 §Outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyViolation {
    /// IRI of the offending step.
    pub step_id: String,
    /// Kind tag literal of the step (e.g. `"http-request"`).
    pub step_kind: String,
    /// Ops the step requires that the env does not allow.
    pub missing_ops: Vec<String>,
    /// IRI of the environment the graph references.
    pub env_id: String,
    /// The env's full `allowedOps` list, in declaration order.
    pub env_allowed_ops: Vec<String>,
    /// The env's safety class literal (e.g. `"production-readonly"`).
    pub env_safety_class: String,
}

impl SafetyViolation {
    /// Render a CLI-friendly diff message.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "step <{step}> (kind `{kind}`) requires {missing:?}, but environment <{env}> (safety class `{class}`) allows only {allowed:?}",
            step = self.step_id,
            kind = self.step_kind,
            missing = self.missing_ops,
            env = self.env_id,
            class = self.env_safety_class,
            allowed = self.env_allowed_ops,
        )
    }
}

impl std::fmt::Display for SafetyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

/// The safety check's failure surface — either a structural ⊆ violation
/// or an unknown op token from either side. Both are recoverable
/// (the StreamWriter aborts the commit) and both carry their own
/// diagnostic context.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SafetyError {
    /// `step.requiredOps ⊄ env.allowedOps`.
    #[error("safety violation: {0}")]
    Violation(SafetyViolation),

    /// A token in `step.requiredOps` or `env.allowedOps` is not in the
    /// controlled vocabulary.
    #[error("unknown op token {token:?} declared by {}", source.as_str())]
    UnknownOp {
        /// The offending token.
        token: String,
        /// Which side of the declaration carries the token. The field
        /// is named `source` per TC-058 §Acceptance 6 — `OpSource`
        /// implements [`std::error::Error`] so thiserror's auto-source
        /// decoration is satisfied.
        source: OpSource,
    },
}

impl SafetyError {
    /// Borrow the underlying violation if the error is a structural ⊆
    /// failure; `None` otherwise.
    #[must_use]
    pub fn as_violation(&self) -> Option<&SafetyViolation> {
        match self {
            Self::Violation(v) => Some(v),
            Self::UnknownOp { .. } => None,
        }
    }
}

/// Resolve a step's `requiredOps` set from its kind + fields. Some
/// kinds (e.g. `sparql-assertion`) are conditional on field contents.
/// Returns the canonical set used by [`check_step_against_env`].
#[must_use]
pub fn required_ops_for(step: &VerificationStep) -> Vec<&'static str> {
    match (&step.kind, &step.fields) {
        (StepKind::ShellCommand, _) => vec!["shell", "filesystem"],
        (StepKind::SparqlAssertion, StepFields::SparqlAssertion { target, .. }) => {
            vec![sparql_op_for_target(target)]
        }
        // Defensive: a misaligned (kind, fields) pair would already
        // fail SHACL, but treat it as a local-store query if the field
        // payload is malformed.
        (StepKind::SparqlAssertion, _) => vec!["sparql-local"],
        (StepKind::FileAssertion, _) => vec!["filesystem"],
        (StepKind::HttpRequest, StepFields::HttpRequest { method, .. }) => {
            vec![http_op_for_method(method)]
        }
        (StepKind::HttpRequest, _) => vec!["http"],
        // `wait-for` is structurally a wrapper. Its required-ops are
        // the union of the wrapped condition's ops; slice 2.5 has no
        // resolver yet so the wrapper itself is treated as safe. The
        // SHACL shape still requires `dec:condition` and `dec:timeout`.
        (StepKind::WaitFor, _) => Vec::new(),
        // `capture` binds a prior step; no ops of its own.
        (StepKind::Capture, _) => Vec::new(),
    }
}

/// Per ADR-028 §Behaviour: a local-file target requires `sparql-local`;
/// an HTTP target requires `sparql-http`.
fn sparql_op_for_target(target: &str) -> &'static str {
    if target.starts_with("http://") || target.starts_with("https://") {
        "sparql-http"
    } else {
        "sparql-local"
    }
}

/// Per ADR-028 §Behaviour: safe verbs (GET/HEAD/OPTIONS) require
/// `http-readonly`; mutating verbs (POST/PUT/PATCH/DELETE) require
/// `http-mutating`. Unknown verbs fall through to the more
/// restrictive `http-mutating`.
fn http_op_for_method(method: &str) -> &'static str {
    let m = method.trim().to_ascii_uppercase();
    match m.as_str() {
        "GET" | "HEAD" | "OPTIONS" => "http-readonly",
        _ => "http-mutating",
    }
}

/// Run the safety check on a single step against an env. The first
/// (and only) violation is returned with full diagnostic context.
/// Unknown ops on either side surface as [`SafetyError::UnknownOp`].
///
/// # Errors
/// Returns [`SafetyError::Violation`] when `step.requiredOps ⊄
/// env.allowedOps`, or [`SafetyError::UnknownOp`] when either side
/// declares a token outside the controlled vocabulary.
pub fn check_step_against_env(
    step: &VerificationStep,
    env: &VerificationBench,
) -> Result<(), SafetyError> {
    validate_op_vocabulary(env)?;
    let required = required_ops_for(step);
    validate_required_ops_vocabulary(&required)?;
    let missing = missing_ops(&required, &env.allowed_ops);
    if missing.is_empty() {
        return Ok(());
    }
    Err(SafetyError::Violation(SafetyViolation {
        step_id: step.id.as_str().to_string(),
        step_kind: step.kind.as_str().to_string(),
        missing_ops: missing,
        env_id: env.iri().as_str().to_string(),
        env_allowed_ops: env.allowed_ops.clone(),
        env_safety_class: env.safety_class.as_str().to_string(),
    }))
}

/// Run the safety check on every step of a graph in order. Returns
/// the **first** violation encountered, mirroring the StreamWriter's
/// fail-fast policy. Use [`check_graph_against_env_all`] for batch
/// authoring tools that want every violation at once.
///
/// An empty graph (no steps) trivially passes (per TC-059 §Acceptance 3).
///
/// # Errors
/// See [`check_step_against_env`]; surfaces the first failing step.
pub fn check_graph_against_env(
    graph: &VerificationGraph,
    env: &VerificationBench,
) -> Result<(), SafetyError> {
    validate_op_vocabulary(env)?;
    for step in &graph.steps {
        check_step_against_env(step, env)?;
    }
    Ok(())
}

/// Run the safety check on every step of a graph and collect every
/// violation in step order. Returns `Ok(())` only if the entire graph
/// is safe. Unknown ops on the env still short-circuit because that
/// is a structural problem independent of individual steps.
///
/// # Errors
/// Returns a vector of every step-level violation. Env-level unknown
/// ops short-circuit and produce a single-element vec carrying
/// [`SafetyError::UnknownOp`].
pub fn check_graph_against_env_all(
    graph: &VerificationGraph,
    env: &VerificationBench,
) -> Result<(), Vec<SafetyError>> {
    if let Err(e) = validate_op_vocabulary(env) {
        return Err(vec![e]);
    }
    let mut errors: Vec<SafetyError> = Vec::new();
    for step in &graph.steps {
        if let Err(e) = check_step_against_env(step, env) {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_op_vocabulary(env: &VerificationBench) -> Result<(), SafetyError> {
    for op in &env.allowed_ops {
        if !is_known_op(op) {
            return Err(SafetyError::UnknownOp {
                token: op.clone(),
                source: OpSource::Env,
            });
        }
    }
    Ok(())
}

fn validate_required_ops_vocabulary(ops: &[&'static str]) -> Result<(), SafetyError> {
    for op in ops {
        if !is_known_op(op) {
            return Err(SafetyError::UnknownOp {
                token: (*op).to_string(),
                source: OpSource::Step,
            });
        }
    }
    Ok(())
}

fn is_known_op(token: &str) -> bool {
    KNOWN_OPS.contains(&token)
}

fn missing_ops(required: &[&str], allowed: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in required {
        if !allowed.iter().any(|a| a == r) {
            out.push((*r).to_string());
        }
    }
    out
}

/// Strongly-typed step IRI for callers that want to surface the IRI
/// in their own error envelopes.
#[must_use]
pub fn violation_step_iri(v: &SafetyViolation) -> StepIri {
    oxigraph::model::NamedNode::new_unchecked(v.step_id.clone())
}
