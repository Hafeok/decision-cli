//! Single-handler discipline shared by every content-management surface (FT-034 / ADR-029).
//!
//! Every content-management subcommand routes through one handler. The
//! handler accepts a [`Request`] (a structured argument shape) and
//! returns `Result<Response, Error>`. Both surfaces — the clap-driven
//! CLI and the MCP server — adapt their wire formats into `Request` and
//! render `Response` / `Error` back out, but the business logic lives
//! exclusively in the handler. This is the structural property ADR-029
//! relies on to keep the two surfaces from drifting.
//!
//! `Request` and `Response` are deliberately thin wrappers around
//! [`serde_json::Value`]. The MCP surface receives a JSON object and
//! passes it through; the CLI surface builds the same object from
//! parsed clap args. Per-tool typed structs live in their own feature
//! modules — `core::handler` exposes only the shared envelope.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::core::verify::safety::SafetyViolation;

/// Arguments to a single tool invocation, passed verbatim from either
/// transport. The shape mirrors MCP's `tools/call.params.arguments` so
/// the MCP surface can forward without translation; the CLI surface
/// constructs the same object from its parsed clap args.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    /// Tool name (e.g. `dec_verify_bench_new`). Carried so handlers that
    /// share an implementation across related tools can branch on it.
    pub tool: String,
    /// JSON object containing the structured arguments. By convention
    /// always a `Value::Object` — non-object values are accepted but
    /// individual handlers may reject them.
    pub arguments: Value,
}

impl Request {
    /// Construct a `Request` for `tool` with the given JSON arguments.
    #[must_use]
    pub fn new(tool: impl Into<String>, arguments: Value) -> Self {
        Self {
            tool: tool.into(),
            arguments,
        }
    }
}

/// Successful tool output. Carries a structured value plus an optional
/// human-readable summary. CLI surfaces print `summary` (or pretty-
/// print `data` when no summary is set); the MCP surface returns the
/// structured `data` as the tool result alongside `summary` as content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Response {
    /// Structured payload, returned verbatim to MCP callers. Always a
    /// JSON value — handlers that have no payload may return `Value::Null`.
    pub data: Value,
    /// Optional human-readable rendering for CLI surfaces. When set,
    /// the MCP surface includes it as the tool's `content` text entry.
    pub summary: Option<String>,
}

impl Response {
    /// Construct a structured-only response (no human summary).
    #[must_use]
    pub fn structured(data: Value) -> Self {
        Self {
            data,
            summary: None,
        }
    }

    /// Construct a response carrying both a structured payload and a
    /// human-readable summary.
    #[must_use]
    pub fn with_summary(data: Value, summary: impl Into<String>) -> Self {
        Self {
            data,
            summary: Some(summary.into()),
        }
    }
}

/// Handler errors. The variants intentionally mirror the structured
/// error families ADR-029 names so both surfaces map onto stable error
/// codes. Per FT-034 §Error handling, MCP renders these as JSON-RPC
/// error envelopes; the CLI renders them on stderr with a non-zero
/// exit code.
#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "code", content = "detail")]
pub enum Error {
    /// The supplied arguments failed validation (missing field, wrong
    /// type, malformed value). Carries the field name and a diagnostic.
    #[error("invalid argument: {field}: {detail}")]
    InvalidArgument {
        /// Name of the offending argument field.
        field: String,
        /// Human-readable explanation.
        detail: String,
    },

    /// The handler could not resolve an artifact the request references
    /// (unknown id, missing file). Carries kind + id for renderers.
    #[error("not found: {kind} '{id}'")]
    ArtifactNotFound {
        /// Artifact kind, e.g. `VerificationBench`.
        kind: String,
        /// Id the caller supplied.
        id: String,
    },

    /// SHACL or other schema validation refused the write.
    #[error("schema violation: {detail}")]
    SchemaViolation {
        /// Human-readable diagnostic (often the SHACL report).
        detail: String,
    },

    /// Caller-supplied id collides with an existing artifact.
    #[error("duplicate id: '{id}'")]
    DuplicateId {
        /// The colliding id.
        id: String,
    },

    /// A reference field (e.g. `dec:verifies`, `dec:environment`) does
    /// not resolve to a known artifact. Per FT-041 / ADR-028, both the
    /// raw reference and the kind of field it appeared in are surfaced
    /// so renderers can present an actionable diagnostic.
    #[error("dangling reference: {kind} ref {reference:?} does not resolve")]
    DanglingRef {
        /// The unresolved reference value as supplied by the caller.
        #[serde(rename = "ref")]
        reference: String,
        /// Which field carried the reference (`"verifies"`, `"environment"`, …).
        kind: String,
    },

    /// A safety constraint refused the operation per FT-037 / ADR-028
    /// §Safety gating: a step's `requiredOps` escaped the environment's
    /// `allowedOps`. Carries the full diagnostic context surfaced by
    /// [`crate::core::verify::safety::SafetyViolation`].
    #[error("safety violation: {0}")]
    SafetyViolation(SafetyViolation),

    /// An op token in either a step's `requiredOps` or an env's
    /// `allowedOps` lies outside the controlled vocabulary
    /// (FT-037 / ADR-028).
    #[error("unknown op token: {token} (source: {origin})")]
    UnknownOp {
        /// The offending op token.
        token: String,
        /// Which side declared it (`"step"` or `"env"`). Named
        /// `origin` rather than `source` to avoid thiserror's
        /// auto-decoration of `source` fields.
        #[serde(rename = "source")]
        origin: String,
    },

    /// The MCP `dec_verify_graph_accept` was invoked with a proposal
    /// whose `bundle_hash` no longer matches the live matcher state
    /// (per FT-049 §Invariants / ADR-030 §Level-3 autonomy). Caller
    /// must re-run `dec_verify_graph_generate`.
    #[error("proposal stale: {detail}")]
    ProposalStale {
        /// Human-readable explanation including the remediation hint.
        detail: String,
    },

    /// FT-107 — the verify-graph-author was dispatched with a non-empty
    /// `defect_feedback` bundle field (the orchestrator deliberately
    /// bypassed the matcher to give the worker a re-authoring
    /// opportunity), but the worker returned `kind = Match`. The accept
    /// path refuses this proposal and emits a meta-feedback artifact
    /// against the worker role itself.
    #[error("worker ignored defect feedback: {detail}")]
    WorkerIgnoredFeedback {
        /// Feedback IRIs the worker failed to address (i.e. the entries
        /// the bundle's `defect_feedback` array contained).
        feedback_iris: Vec<String>,
        /// Human-readable explanation including the remediation hint.
        detail: String,
    },

    /// Catch-all for IO / runtime / unexpected failures the handler
    /// surfaces as a generic error.
    #[error("internal error: {detail}")]
    Internal {
        /// Free-form diagnostic.
        detail: String,
    },
}

impl From<crate::core::verify::safety::SafetyError> for Error {
    fn from(value: crate::core::verify::safety::SafetyError) -> Self {
        use crate::core::verify::safety::SafetyError;
        match value {
            SafetyError::Violation(v) => Self::SafetyViolation(v),
            SafetyError::UnknownOp { token, source } => Self::UnknownOp {
                token,
                origin: source.as_str().to_string(),
            },
        }
    }
}

impl Error {
    /// Stable error-code string used by MCP envelopes and CLI exit
    /// telemetry. Mirrors the `code` field of the serde tag.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "InvalidArgument",
            Self::ArtifactNotFound { .. } => "ArtifactNotFound",
            Self::SchemaViolation { .. } => "SchemaViolation",
            Self::DuplicateId { .. } => "DuplicateId",
            Self::DanglingRef { .. } => "DanglingRef",
            Self::SafetyViolation(_) => "SafetyViolation",
            Self::UnknownOp { .. } => "UnknownOp",
            Self::ProposalStale { .. } => "ProposalStale",
            Self::WorkerIgnoredFeedback { .. } => "WorkerIgnoredFeedback",
            Self::Internal { .. } => "Internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrips_through_json() {
        let req = Request::new("dec_mcp_ping", serde_json::json!({"echo": "hi"}));
        let raw = serde_json::to_string(&req).expect("serialise");
        let back: Request = serde_json::from_str(&raw).expect("deserialise");
        assert_eq!(req, back);
    }

    #[test]
    fn response_summary_optional() {
        let r1 = Response::structured(serde_json::json!({"ok": true}));
        assert!(r1.summary.is_none());
        let r2 = Response::with_summary(serde_json::json!({"ok": true}), "ok");
        assert_eq!(r2.summary.as_deref(), Some("ok"));
    }

    #[test]
    fn error_codes_are_stable() {
        let err = Error::InvalidArgument {
            field: "id".to_string(),
            detail: "blank".to_string(),
        };
        assert_eq!(err.code(), "InvalidArgument");
        let raw = serde_json::to_string(&err).expect("serialise");
        // Tag-and-content encoding for downstream MCP envelopes.
        assert!(raw.contains("\"code\":\"InvalidArgument\""));
    }
}
