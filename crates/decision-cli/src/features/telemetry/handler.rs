//! axum router for `/llm-call-telemetry` reconciliation endpoint (FT-096).
//!
//! Two-layer split mirroring `features::submissions`:
//!
//! - [`TelemetryService`] — transport-free logic: take bearer token plus
//!   payload, return typed `Result<TelemetryAccepted, TelemetryRejection>`.
//! - [`router`] / [`AppState`] — axum-side glue mapping typed errors to
//!   HTTP status codes (401 / 422 / 500).
//!
//! Splitting like this lets the TC-138 integration test drive the
//! service in isolation while still exercising the router end-to-end
//! through `tower::ServiceExt::oneshot`.

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::payload::TelemetryPayload;
use super::store::TelemetryStore;

/// Service-layer error variants. Each maps 1:1 to an HTTP status code
/// in the axum adapter, but the service stays transport-free so callers
/// can drive it from non-HTTP harnesses (CLI ingest, integration tests).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TelemetryServiceError {
    /// Bearer token missing or unknown — 401.
    #[error("unauthorised: bearer token missing or unknown")]
    Unauthorised,
    /// Payload `ddd_session_id` is empty — 422.
    #[error("payload invalid: ddd_session_id must be non-empty")]
    MissingSessionId,
    /// Underlying store failure — 500.
    #[error("internal: {detail}")]
    Internal {
        /// Diagnostic detail; not surfaced to the network.
        detail: String,
    },
}

/// Decision returned by [`TelemetryService::accept`] on the success path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryAccepted {
    /// Echo of the indexed `ddd_session_id`.
    pub ddd_session_id: String,
    /// Echo of the capability tag the call routed through.
    pub capability_tag: String,
}

/// Body shape returned on a rejection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryRejection {
    /// Machine-readable error tag.
    pub error: String,
    /// Operator-facing detail.
    pub detail: String,
}

/// Service handle. Encapsulates the expected bearer token plus the
/// in-memory telemetry store.
#[derive(Debug, Clone)]
pub struct TelemetryService {
    expected_token: String,
    store: TelemetryStore,
}

impl TelemetryService {
    /// Construct a service over the given dependencies.
    #[must_use]
    pub fn new(expected_token: impl Into<String>, store: TelemetryStore) -> Self {
        Self {
            expected_token: expected_token.into(),
            store,
        }
    }

    /// Borrow the underlying store. Used by `dec` inspection commands to
    /// reconcile session cost figures.
    #[must_use]
    pub fn store(&self) -> &TelemetryStore {
        &self.store
    }

    /// Run the full ingest lifecycle for one telemetry POST.
    pub fn accept(
        &self,
        bearer: Option<&str>,
        payload: TelemetryPayload,
    ) -> Result<TelemetryAccepted, TelemetryServiceError> {
        self.authenticate(bearer)?;
        if !payload.has_session_id() {
            return Err(TelemetryServiceError::MissingSessionId);
        }
        let accepted = TelemetryAccepted {
            ddd_session_id: payload.ddd_session_id.clone(),
            capability_tag: payload.capability_tag.clone(),
        };
        self.store
            .record(payload)
            .map_err(|err| TelemetryServiceError::Internal {
                detail: err.to_string(),
            })?;
        Ok(accepted)
    }

    fn authenticate(&self, bearer: Option<&str>) -> Result<(), TelemetryServiceError> {
        let raw = bearer.ok_or(TelemetryServiceError::Unauthorised)?;
        if raw == self.expected_token {
            Ok(())
        } else {
            Err(TelemetryServiceError::Unauthorised)
        }
    }
}

/// State threaded through axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// The pre-built telemetry service.
    pub service: TelemetryService,
}

/// Build the axum router that exposes `POST /llm-call-telemetry`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/llm-call-telemetry", post(handle_post))
        .with_state(state)
}

async fn handle_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<TelemetryPayload>,
) -> Response {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let token = parse_bearer(auth);
    match state.service.accept(token, payload) {
        Ok(accepted) => (StatusCode::OK, Json(accepted)).into_response(),
        Err(err) => render_rejection(&err).into_response(),
    }
}

fn parse_bearer(header_value: Option<&str>) -> Option<&str> {
    let raw = header_value?;
    let stripped = raw.strip_prefix("Bearer ")?;
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

fn render_rejection(err: &TelemetryServiceError) -> (StatusCode, Json<TelemetryRejection>) {
    let (status, tag, detail) = match err {
        TelemetryServiceError::Unauthorised => (
            StatusCode::UNAUTHORIZED,
            "unauthorised",
            "bearer token missing or unknown".to_string(),
        ),
        TelemetryServiceError::MissingSessionId => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "missing_session_id",
            "payload.ddd_session_id must be non-empty".to_string(),
        ),
        TelemetryServiceError::Internal { detail } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            detail.clone(),
        ),
    };
    (
        status,
        Json(TelemetryRejection {
            error: tag.to_string(),
            detail,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(session: &str) -> TelemetryPayload {
        TelemetryPayload {
            ddd_session_id: session.to_string(),
            model: "frontier-reasoning".to_string(),
            provider: "anthropic".to_string(),
            capability_tag: "frontier-reasoning".to_string(),
            input_tokens: 12,
            output_tokens: 34,
            cost_usd: 0.005,
            latency_ms: 410,
            retry_count: 0,
            fallback_chain: vec![],
        }
    }

    #[test]
    fn service_rejects_missing_bearer() {
        let svc = TelemetryService::new("tok", TelemetryStore::new());
        let err = svc.accept(None, fixture("s")).expect_err("must reject");
        assert_eq!(err, TelemetryServiceError::Unauthorised);
    }

    #[test]
    fn service_rejects_wrong_bearer() {
        let svc = TelemetryService::new("tok", TelemetryStore::new());
        let err = svc
            .accept(Some("wrong"), fixture("s"))
            .expect_err("must reject");
        assert_eq!(err, TelemetryServiceError::Unauthorised);
    }

    #[test]
    fn service_rejects_empty_session_id() {
        let svc = TelemetryService::new("tok", TelemetryStore::new());
        let err = svc.accept(Some("tok"), fixture("")).expect_err("must reject");
        assert_eq!(err, TelemetryServiceError::MissingSessionId);
    }

    #[test]
    fn service_indexes_record_under_session_id() {
        let store = TelemetryStore::new();
        let svc = TelemetryService::new("tok", store.clone());
        let accepted = svc.accept(Some("tok"), fixture("s-7")).expect("accept");
        assert_eq!(accepted.ddd_session_id, "s-7");
        let cost = store.total_cost_usd("s-7").expect("total");
        assert!((cost - 0.005).abs() < 1e-9);
    }

    #[test]
    fn parse_bearer_accepts_well_formed_header() {
        assert_eq!(parse_bearer(Some("Bearer abc")), Some("abc"));
    }

    #[test]
    fn parse_bearer_rejects_empty_payload() {
        assert_eq!(parse_bearer(Some("Bearer ")), None);
    }
}
