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
    validate_env_type(&req.env_type)?;
    validate_safety_class(&req.safety_class)?;
    validate_allowed_ops(&req.allowed_ops)?;
    validate_endpoint(&req.env_type, req.endpoint.as_deref())?;
    validate_fixture_source(req.fixture_source.as_deref(), req.workdir.as_deref())?;
    if let Some(id) = &req.id {
        validate_id_format(id)?;
    }
    Ok(())
}

/// FT-053 / ADR-032 — `fixture_source` must be repo-relative and point
/// at an existing directory under the workdir. `None` is always valid.
fn validate_fixture_source(
    fixture_source: Option<&str>,
    workdir: Option<&std::path::Path>,
) -> Result<(), HandlerError> {
    let Some(raw) = fixture_source else {
        return Ok(());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "fixture_source".to_string(),
            detail: "fixture_source must be a non-empty string".to_string(),
        });
    }
    if trimmed.starts_with('/') {
        return Err(HandlerError::InvalidArgument {
            field: "fixture_source".to_string(),
            detail: "fixture_source must be repo-relative".to_string(),
        });
    }
    let p = std::path::Path::new(trimmed);
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(HandlerError::InvalidArgument {
            field: "fixture_source".to_string(),
            detail: "fixture_source must not contain `..`".to_string(),
        });
    }
    let Some(workdir) = workdir else {
        return Ok(());
    };
    let resolved = workdir.join(trimmed);
    if !resolved.exists() {
        return Err(HandlerError::InvalidArgument {
            field: "fixture_source".to_string(),
            detail: format!("fixture_source {trimmed:?} does not exist"),
        });
    }
    if !resolved.is_dir() {
        return Err(HandlerError::InvalidArgument {
            field: "fixture_source".to_string(),
            detail: format!("fixture_source {trimmed:?} is not a directory"),
        });
    }
    Ok(())
}

/// `env-type` must be a non-empty string (whitespace doesn't count).
fn validate_env_type(env_type: &str) -> Result<(), HandlerError> {
    if env_type.trim().is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "env_type".to_string(),
            detail: "env-type must be a non-empty string".to_string(),
        });
    }
    Ok(())
}

/// `safety-class` must be one of the three controlled values.
fn validate_safety_class(safety_class: &str) -> Result<(), HandlerError> {
    if SafetyClass::parse(safety_class).is_some() {
        return Ok(());
    }
    Err(HandlerError::InvalidArgument {
        field: "safety_class".to_string(),
        detail: format!(
            "safety-class must be one of {{{SAFETY_ISOLATED}, \
             {SAFETY_SHARED_NON_DESTRUCTIVE}, {SAFETY_PRODUCTION_READONLY}}}; \
             got {got:?}",
            got = safety_class,
        ),
    })
}

/// `allowed-ops` must be a non-empty list of non-empty tokens.
fn validate_allowed_ops(ops: &[String]) -> Result<(), HandlerError> {
    if ops.is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "allowed_ops".to_string(),
            detail: "allowed-ops must contain at least one operation token".to_string(),
        });
    }
    for op in ops {
        if op.trim().is_empty() {
            return Err(HandlerError::InvalidArgument {
                field: "allowed_ops".to_string(),
                detail: "allowed-ops tokens must be non-empty strings".to_string(),
            });
        }
    }
    Ok(())
}

/// `--endpoint` is required for `remote-*` env types, forbidden otherwise.
fn validate_endpoint(env_type: &str, endpoint: Option<&str>) -> Result<(), HandlerError> {
    let is_remote = env_type.starts_with(REMOTE_ENV_TYPE_PREFIX);
    let endpoint_present = endpoint.is_some_and(|s| !s.is_empty());
    if is_remote && !endpoint_present {
        return Err(HandlerError::InvalidArgument {
            field: "endpoint".to_string(),
            detail: format!(
                "remote env types (env-type starts with {REMOTE_ENV_TYPE_PREFIX:?}) require --endpoint"
            ),
        });
    }
    if !is_remote && endpoint.is_some() {
        return Err(HandlerError::InvalidArgument {
            field: "endpoint".to_string(),
            detail: format!(
                "local env types (env-type does not start with {REMOTE_ENV_TYPE_PREFIX:?}) must NOT carry --endpoint"
            ),
        });
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
            fixture_source: None,
            workdir: None,
        }
    }

    /// FT-053: absolute paths are rejected.
    #[test]
    fn rejects_absolute_fixture_source() {
        let mut req = ephemeral_req();
        req.fixture_source = Some("/etc".to_string());
        let err = pre_validate(&req).expect_err("absolute fixture_source must fail");
        match err {
            HandlerError::InvalidArgument { field, detail } => {
                assert_eq!(field, "fixture_source");
                assert!(detail.contains("repo-relative"), "{detail}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// FT-053: `..` segments are rejected.
    #[test]
    fn rejects_parent_dir_fixture_source() {
        let mut req = ephemeral_req();
        req.fixture_source = Some("foo/../bar".to_string());
        let err = pre_validate(&req).expect_err("`..` fixture_source must fail");
        match err {
            HandlerError::InvalidArgument { field, detail } => {
                assert_eq!(field, "fixture_source");
                assert!(detail.contains(".."), "{detail}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// FT-053: workdir-relative path that doesn't resolve is rejected
    /// (only when a workdir is set).
    #[test]
    fn rejects_missing_fixture_source_dir() {
        let mut req = ephemeral_req();
        req.fixture_source = Some("tests/fixtures/__does_not_exist__".to_string());
        req.workdir = Some(std::env::temp_dir());
        let err = pre_validate(&req).expect_err("missing dir must fail");
        match err {
            HandlerError::InvalidArgument { field, detail } => {
                assert_eq!(field, "fixture_source");
                assert!(detail.contains("does not exist"), "{detail}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// FT-053: a real directory under workdir passes.
    #[test]
    fn accepts_existing_fixture_source_dir() {
        let tmp = std::env::temp_dir();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let fixture = tmp.join(format!("dec-fixture-validate-{nonce}"));
        std::fs::create_dir_all(&fixture).expect("mkdir");
        let mut req = ephemeral_req();
        req.fixture_source = Some(
            fixture
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned(),
        );
        req.workdir = Some(tmp.clone());
        let result = pre_validate(&req);
        let _ = std::fs::remove_dir_all(&fixture);
        result.expect("real dir under workdir must pass");
    }

    /// FT-053: a file (not directory) is rejected.
    #[test]
    fn rejects_fixture_source_pointing_at_file() {
        let tmp = std::env::temp_dir();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = tmp.join(format!("dec-fixture-validate-file-{nonce}"));
        std::fs::write(&path, b"not a directory").expect("touch");
        let mut req = ephemeral_req();
        req.fixture_source = Some(
            path.file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned(),
        );
        req.workdir = Some(tmp.clone());
        let err = pre_validate(&req).expect_err("file path must fail");
        let _ = std::fs::remove_file(&path);
        match err {
            HandlerError::InvalidArgument { field, detail } => {
                assert_eq!(field, "fixture_source");
                assert!(detail.contains("not a directory"), "{detail}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
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
