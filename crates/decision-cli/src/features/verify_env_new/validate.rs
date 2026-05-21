//! Pre-write input validation for `dec verify env new` (FT-038).
//!
//! Mirrors FT-038 §Behaviour step 2: env-type non-empty, safety-class
//! in the controlled list, allowed-ops non-empty, endpoint required iff
//! the env-type matches `remote-*`, caller-supplied ids well-formed.
//!
//! Each failure maps to [`HandlerError::InvalidArgument`] naming the
//! offending field so the CLI can exit with code 2 (usage error) and
//! the MCP surface returns a structured error.

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_env::{SafetyClass, REMOTE_ENV_TYPE_PREFIX};
use crate::core::vocab::{
    SAFETY_ISOLATED, SAFETY_PRODUCTION_READONLY, SAFETY_SHARED_NON_DESTRUCTIVE,
};

use super::EnvNewRequest;

/// Validate the request before any I/O.
pub(super) fn pre_validate(req: &EnvNewRequest) -> Result<(), HandlerError> {
    if req.env_type.trim().is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "env_type".to_string(),
            detail: "env-type must be a non-empty string".to_string(),
        });
    }
    if SafetyClass::parse(&req.safety_class).is_none() {
        return Err(HandlerError::InvalidArgument {
            field: "safety_class".to_string(),
            detail: format!(
                "safety-class must be one of {{{SAFETY_ISOLATED}, \
                 {SAFETY_SHARED_NON_DESTRUCTIVE}, {SAFETY_PRODUCTION_READONLY}}}; \
                 got {got:?}",
                got = req.safety_class,
            ),
        });
    }
    if req.allowed_ops.is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "allowed_ops".to_string(),
            detail: "allowed-ops must contain at least one operation token".to_string(),
        });
    }
    for op in &req.allowed_ops {
        if op.trim().is_empty() {
            return Err(HandlerError::InvalidArgument {
                field: "allowed_ops".to_string(),
                detail: "allowed-ops tokens must be non-empty strings".to_string(),
            });
        }
    }
    let is_remote = req.env_type.starts_with(REMOTE_ENV_TYPE_PREFIX);
    let endpoint_present = req.endpoint.as_deref().is_some_and(|s| !s.is_empty());
    if is_remote && !endpoint_present {
        return Err(HandlerError::InvalidArgument {
            field: "endpoint".to_string(),
            detail: format!(
                "remote env types (env-type starts with {REMOTE_ENV_TYPE_PREFIX:?}) require --endpoint"
            ),
        });
    }
    if !is_remote && req.endpoint.is_some() {
        return Err(HandlerError::InvalidArgument {
            field: "endpoint".to_string(),
            detail: format!(
                "local env types (env-type does not start with {REMOTE_ENV_TYPE_PREFIX:?}) must NOT carry --endpoint"
            ),
        });
    }
    if let Some(id) = &req.id {
        validate_id_format(id)?;
    }
    Ok(())
}

fn validate_id_format(id: &str) -> Result<(), HandlerError> {
    if !id.starts_with("ENV-") {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("env id must start with 'ENV-', got {id:?}"),
        });
    }
    if id.len() < 5 {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("env id {id:?} is too short (expected ENV-NNN[-suffix])"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ephemeral_req() -> EnvNewRequest {
        EnvNewRequest {
            id: None,
            env_type: "ephemeral-tempdir".to_string(),
            safety_class: SAFETY_ISOLATED.to_string(),
            allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
            setup: None,
            teardown: None,
            endpoint: None,
            workdir: None,
        }
    }

    #[test]
    fn rejects_unknown_safety_class() {
        let mut req = ephemeral_req();
        req.safety_class = "yolo".to_string();
        let err = pre_validate(&req).expect_err("unknown safety class must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "safety_class"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn rejects_empty_allowed_ops() {
        let mut req = ephemeral_req();
        req.allowed_ops.clear();
        let err = pre_validate(&req).expect_err("empty allowed_ops must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "allowed_ops"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn rejects_remote_without_endpoint() {
        let mut req = ephemeral_req();
        req.env_type = "remote-http".to_string();
        req.safety_class = SAFETY_SHARED_NON_DESTRUCTIVE.to_string();
        req.allowed_ops = vec!["http".to_string()];
        let err = pre_validate(&req).expect_err("remote without endpoint must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "endpoint"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn rejects_local_with_endpoint() {
        let mut req = ephemeral_req();
        req.endpoint = Some("https://example.com".to_string());
        let err = pre_validate(&req).expect_err("local with endpoint must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "endpoint"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn accepts_remote_with_endpoint() {
        let mut req = ephemeral_req();
        req.env_type = "remote-http".to_string();
        req.safety_class = SAFETY_SHARED_NON_DESTRUCTIVE.to_string();
        req.allowed_ops = vec!["http".to_string()];
        req.endpoint = Some("https://example.com".to_string());
        pre_validate(&req).expect("remote with endpoint must pass");
    }

    #[test]
    fn rejects_malformed_id() {
        let mut req = ephemeral_req();
        req.id = Some("ENV".to_string());
        let err = pre_validate(&req).expect_err("malformed id must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }
}
