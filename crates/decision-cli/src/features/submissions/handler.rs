//! axum handler + service layer for `POST /submissions` (FT-094).
//!
//! The handler is split into three concerns the test suite drives
//! independently:
//!
//! - [`SubmissionsService`] — pure transport-free logic: take a parsed
//!   payload + bearer token, return a typed `Result<SubmissionAccepted,
//!   SubmissionRejection>`. No `axum::http` types leak in.
//! - [`router`] / [`AppState`] — the axum-side glue. Maps the typed
//!   `SubmissionRejection` to HTTP status codes (401/403/422/429).
//! - [`RateLimiter`] — a per-repo token-bucket with a fixed refill rate.
//!   Slice 1 ships a loose default (60 req/min per repo); slice 3+
//!   makes it policy-driven.
//!
//! Splitting like this lets the integration tests drive the service in
//! isolation (no socket, no JSON round-trip) while still exercising the
//! axum router end-to-end through `tower::ServiceExt::oneshot`.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use oxi_events::{GraphWriter, Mutation};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::core::ontology::boundary_artifact::validate_boundary_artifact;
use crate::core::ontology::worker_image_submission::{validate_quads, WorkerImageSubmission};
use crate::core::vocab::worker_image_submission_graph;

use super::auth::{parse_bearer, RepoIdentity, TokenStore};
use super::payload::SubmissionPayload;
pub use super::rate_limit::RateLimiter;

/// Service-layer error variants. Each maps 1:1 to an HTTP status code
/// in the axum adapter, but the service itself stays transport-free so
/// callers can use it from non-HTTP harnesses (CLI ingest, dogfood
/// scripts).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SubmissionsServiceError {
    /// Bearer token missing or unknown — 401.
    #[error("unauthorised: bearer token missing or unknown")]
    Unauthorised,
    /// Bearer token resolved, but the declared `claimed_source_repo_uri`
    /// does not match the bound identity — 403.
    #[error(
        "forbidden: token identity <{token_identity}> does not match declared source repo <{declared}>"
    )]
    IdentityMismatch {
        /// The token's bound repo identity.
        token_identity: String,
        /// The Submission's declared `claimed_source_repo_uri`.
        declared: String,
    },
    /// SHACL validation failed (or the payload's `claimed_compatible_roles`
    /// list contained a malformed IRI) — 422.
    #[error("payload validation failed: {report}")]
    PayloadInvalid {
        /// Rendered violations (one line per violation).
        report: String,
    },
    /// Per-repo rate limit exceeded — 429.
    #[error("rate limit exceeded for repo <{repo}>")]
    RateLimited {
        /// The throttled repo identity.
        repo: String,
    },
    /// Underlying store failure surfaces as a 500-equivalent; the
    /// service does NOT translate this to a typed HTTP status because
    /// it is exceptional. The axum adapter maps it to 500.
    #[error("internal: {detail}")]
    Internal {
        /// Diagnostic detail; not surfaced to the network.
        detail: String,
    },
}

/// Decision returned by [`SubmissionsService::accept`] on the success
/// path. The caller (axum adapter or non-HTTP harness) renders this
/// into a response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionAccepted {
    /// Canonical Submission IRI written into the orchestration store.
    pub submission_iri: String,
    /// Submission id (the trailing segment of `submission_iri`).
    pub submission_id: String,
    /// Synthetic dispatch-event id the `WorkerCurator` subscription will
    /// consume. Slice-1 mints a `UUIDv4`; slice-2+ uses the `GraphWriter`'s
    /// emitted `EventHandle.iri` once subscription evaluation lands on
    /// the wire (FT-092 also expects this).
    pub dispatch_event_id: String,
    /// Target role for the dispatched event — fixed at `worker-curator`.
    pub target_role: String,
}

/// Body shape returned on a rejection. Slice 1 keeps it lossy on
/// purpose: external CI consumers see status codes + violation
/// summaries, not raw SHACL graphs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionRejection {
    /// Short machine-readable error tag matching the variant name
    /// (`unauthorised`, `identity_mismatch`, `payload_invalid`,
    /// `rate_limited`).
    pub error: String,
    /// Operator-facing detail. For 422 this lists each failed field;
    /// for 401/403/429 it is a short phrase.
    pub detail: String,
}

/// Service handle the axum adapter dispatches into. Encapsulates the
/// `GraphWriter`, token store, and rate limiter behind a single
/// `accept` method.
#[derive(Clone)]
pub struct SubmissionsService {
    writer: Arc<GraphWriter>,
    tokens: TokenStore,
    limiter: Arc<RateLimiter>,
}

impl SubmissionsService {
    /// Build a service over the given dependencies.
    #[must_use]
    pub fn new(writer: Arc<GraphWriter>, tokens: TokenStore, limiter: Arc<RateLimiter>) -> Self {
        Self {
            writer,
            tokens,
            limiter,
        }
    }

    /// Run the full submission lifecycle. Errors at any stage abort the
    /// transaction; no partial state lands in the orchestration store.
    pub fn accept(
        &self,
        bearer_token: Option<&str>,
        payload: SubmissionPayload,
    ) -> Result<SubmissionAccepted, SubmissionsServiceError> {
        let identity = self.authenticate(bearer_token)?;
        self.enforce_rate_limit(&identity)?;
        let submission = build_submission(payload, &identity)?;
        let quads = serialise_and_validate(&submission)?;
        let cause = format!("FT-094: POST /submissions from <{}>", identity.as_str());
        let mutation = Mutation::insert(quads).with_cause(cause);
        self.writer
            .commit(mutation)
            .map_err(|err| SubmissionsServiceError::Internal {
                detail: format!("graph commit: {err}"),
            })?;
        let dispatch_event_id = format!("urn:uuid:{}", Uuid::new_v4());
        Ok(SubmissionAccepted {
            submission_iri: submission.iri().as_str().to_string(),
            submission_id: submission.id.clone(),
            dispatch_event_id,
            target_role: "worker-curator".to_string(),
        })
    }

    fn authenticate(
        &self,
        token: Option<&str>,
    ) -> Result<RepoIdentity, SubmissionsServiceError> {
        let raw = token.ok_or(SubmissionsServiceError::Unauthorised)?;
        self.tokens
            .resolve(raw)
            .ok_or(SubmissionsServiceError::Unauthorised)
    }

    fn enforce_rate_limit(
        &self,
        identity: &RepoIdentity,
    ) -> Result<(), SubmissionsServiceError> {
        if self.limiter.try_acquire(identity) {
            Ok(())
        } else {
            Err(SubmissionsServiceError::RateLimited {
                repo: identity.as_str().to_string(),
            })
        }
    }
}

fn build_submission(
    payload: SubmissionPayload,
    identity: &RepoIdentity,
) -> Result<WorkerImageSubmission, SubmissionsServiceError> {
    if !identity.matches_declared(&payload.claimed_source_repo_uri) {
        return Err(SubmissionsServiceError::IdentityMismatch {
            token_identity: identity.as_str().to_string(),
            declared: payload.claimed_source_repo_uri.clone(),
        });
    }
    payload
        .into_submission(|| Uuid::new_v4().to_string())
        .map_err(|err| SubmissionsServiceError::PayloadInvalid {
            report: format!(
                "claimed_compatible_roles entry {:?} is not a valid IRI: {}",
                err.offender, err.detail
            ),
        })
}

fn serialise_and_validate(
    submission: &WorkerImageSubmission,
) -> Result<Vec<oxigraph::model::Quad>, SubmissionsServiceError> {
    let quads = submission.to_quads(worker_image_submission_graph());
    validate_quads(&quads).map_err(|err| SubmissionsServiceError::PayloadInvalid {
        report: err.report,
    })?;
    validate_boundary_artifact(&quads, &submission.iri()).map_err(|err| {
        SubmissionsServiceError::PayloadInvalid {
            report: err.report,
        }
    })?;
    Ok(quads)
}

/// State threaded through axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// The pre-built submissions service.
    pub service: SubmissionsService,
}

/// Build the axum router that exposes `POST /submissions`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/submissions", post(handle_post))
        .with_state(state)
}

async fn handle_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SubmissionPayload>,
) -> Response {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let token = parse_bearer(auth_header);
    match state.service.accept(token, payload) {
        Ok(accepted) => (StatusCode::OK, Json(accepted)).into_response(),
        Err(err) => render_rejection(&err).into_response(),
    }
}

fn render_rejection(err: &SubmissionsServiceError) -> (StatusCode, Json<SubmissionRejection>) {
    let (status, error, detail) = match err {
        SubmissionsServiceError::Unauthorised => (
            StatusCode::UNAUTHORIZED,
            "unauthorised",
            "bearer token missing or unknown".to_string(),
        ),
        SubmissionsServiceError::IdentityMismatch {
            token_identity,
            declared,
        } => (
            StatusCode::FORBIDDEN,
            "identity_mismatch",
            format!(
                "token identity <{token_identity}> does not match declared source repo <{declared}>"
            ),
        ),
        SubmissionsServiceError::PayloadInvalid { report } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "payload_invalid",
            report.clone(),
        ),
        SubmissionsServiceError::RateLimited { repo } => (
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            format!("rate limit exceeded for repo <{repo}>"),
        ),
        SubmissionsServiceError::Internal { detail } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            detail.clone(),
        ),
    };
    (
        status,
        Json(SubmissionRejection {
            error: error.to_string(),
            detail,
        }),
    )
}

#[cfg(test)]
mod handler_unit_tests {
    use super::*;

    #[test]
    fn rejection_render_unauthorised_maps_to_401() {
        let (status, body) =
            render_rejection(&SubmissionsServiceError::Unauthorised);
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.0.error, "unauthorised");
    }

    #[test]
    fn rejection_render_identity_mismatch_maps_to_403() {
        let (status, body) = render_rejection(&SubmissionsServiceError::IdentityMismatch {
            token_identity: "tok".into(),
            declared: "decl".into(),
        });
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body.0.error, "identity_mismatch");
    }

    #[test]
    fn rejection_render_payload_invalid_maps_to_422() {
        let (status, body) = render_rejection(&SubmissionsServiceError::PayloadInvalid {
            report: "fields x, y".into(),
        });
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.0.error, "payload_invalid");
        assert_eq!(body.0.detail, "fields x, y");
    }

    #[test]
    fn rejection_render_rate_limited_maps_to_429() {
        let (status, body) = render_rejection(&SubmissionsServiceError::RateLimited {
            repo: "r".into(),
        });
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body.0.error, "rate_limited");
    }
}
