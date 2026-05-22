//! Parse the caller-supplied `fields` map into a typed `StepFields`
//! discriminated union (FT-044).
//!
//! Two-stage validation per FT-044 §Behaviour:
//!
//!   1. Unknown keys for the supplied step kind surface as
//!      `Error::InvalidArgument { field: "fields.<key>" }` (exit 2).
//!   2. Missing required keys surface as `Error::SchemaViolation`
//!      (exit 1) — the field's predicate name appears in the detail so
//!      the diagnostic round-trips with FT-036's SHACL-driven errors.
//!
//! `${name}` placeholders are accepted verbatim — FT-044 §Invariants:
//! "no resolution and no validation against capture availability".

use std::collections::BTreeMap;

use oxigraph::model::NamedNode;

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_graph::{StepFields, StepKind};

/// Parse the caller-supplied `fields` map into a typed [`StepFields`]
/// value matching `kind`. Returns:
///   * `InvalidArgument { field: "fields.<key>" }` for unknown keys.
///   * `SchemaViolation { detail }` for missing required keys.
pub(super) fn build_step_fields(
    kind: StepKind,
    fields: &BTreeMap<String, String>,
) -> Result<StepFields, HandlerError> {
    reject_unknown_keys(kind, fields)?;
    match kind {
        StepKind::ShellCommand => build_shell_command(fields),
        StepKind::SparqlAssertion => build_sparql_assertion(fields),
        StepKind::FileAssertion => build_file_assertion(fields),
        StepKind::HttpRequest => build_http_request(fields),
        StepKind::WaitFor => build_wait_for(fields),
        StepKind::Capture => build_capture(fields),
    }
}

/// Stable list of every accepted `--field` key per step kind. Keys
/// outside this list surface as `InvalidArgument`. Kept here so the CLI
/// help text, MCP schema, and validation share one source of truth.
#[must_use]
pub(super) fn known_keys(kind: StepKind) -> &'static [&'static str] {
    match kind {
        StepKind::ShellCommand => &["command", "expect-exit-code", "capture-output"],
        StepKind::SparqlAssertion => &["target", "query", "expect-rows"],
        StepKind::FileAssertion => &["path", "expect-hash", "expect-content"],
        StepKind::HttpRequest => &["method", "url", "expect-status"],
        StepKind::WaitFor => &["condition", "timeout"],
        StepKind::Capture => &["bind-as", "from-step"],
    }
}

fn reject_unknown_keys(
    kind: StepKind,
    fields: &BTreeMap<String, String>,
) -> Result<(), HandlerError> {
    let allowed = known_keys(kind);
    for key in fields.keys() {
        if !allowed.iter().any(|k| *k == key.as_str()) {
            return Err(HandlerError::InvalidArgument {
                field: format!("fields.{key}"),
                detail: format!(
                    "unknown field {key:?} for step kind {kind:?}; allowed: {allowed:?}"
                ),
            });
        }
    }
    Ok(())
}

fn build_shell_command(fields: &BTreeMap<String, String>) -> Result<StepFields, HandlerError> {
    let command = required(fields, "command", "shell-command", "dec:command")?;
    let expect_exit_code = optional_i64(fields, "expect-exit-code")?;
    let capture_output = optional_bool(fields, "capture-output")?;
    Ok(StepFields::ShellCommand {
        command,
        expect_exit_code,
        capture_output,
    })
}

fn build_sparql_assertion(fields: &BTreeMap<String, String>) -> Result<StepFields, HandlerError> {
    let target = required(fields, "target", "sparql-assertion", "dec:target")?;
    let query = required(fields, "query", "sparql-assertion", "dec:query")?;
    let expect_rows = optional_i64(fields, "expect-rows")?;
    Ok(StepFields::SparqlAssertion {
        target,
        query,
        expect_rows,
    })
}

fn build_file_assertion(fields: &BTreeMap<String, String>) -> Result<StepFields, HandlerError> {
    let path = required(fields, "path", "file-assertion", "dec:path")?;
    let expect_hash = fields.get("expect-hash").cloned();
    let expect_content = fields.get("expect-content").cloned();
    Ok(StepFields::FileAssertion {
        path,
        expect_hash,
        expect_content,
    })
}

fn build_http_request(fields: &BTreeMap<String, String>) -> Result<StepFields, HandlerError> {
    let method = required(fields, "method", "http-request", "dec:method")?;
    let url = required(fields, "url", "http-request", "dec:url")?;
    let expect_status = optional_i64(fields, "expect-status")?;
    Ok(StepFields::HttpRequest {
        method,
        url,
        expect_status,
    })
}

fn build_wait_for(fields: &BTreeMap<String, String>) -> Result<StepFields, HandlerError> {
    let condition_raw = required(fields, "condition", "wait-for", "dec:condition")?;
    let timeout = required(fields, "timeout", "wait-for", "dec:timeout")?;
    let condition =
        NamedNode::new(condition_raw.as_str()).map_err(|e| HandlerError::InvalidArgument {
            field: "fields.condition".to_string(),
            detail: format!("condition must be an IRI: {e}"),
        })?;
    Ok(StepFields::WaitFor { condition, timeout })
}

fn build_capture(fields: &BTreeMap<String, String>) -> Result<StepFields, HandlerError> {
    let bind_as = required(fields, "bind-as", "capture", "dec:bindAs")?;
    let from_step = match fields.get("from-step") {
        Some(raw) => {
            Some(
                NamedNode::new(raw.as_str()).map_err(|e| HandlerError::InvalidArgument {
                    field: "fields.from-step".to_string(),
                    detail: format!("from-step must be an IRI: {e}"),
                })?,
            )
        }
        None => None,
    };
    Ok(StepFields::Capture { from_step, bind_as })
}

fn required(
    fields: &BTreeMap<String, String>,
    key: &str,
    kind_label: &str,
    predicate: &str,
) -> Result<String, HandlerError> {
    match fields.get(key) {
        Some(v) if !v.is_empty() => Ok(v.clone()),
        _ => Err(HandlerError::SchemaViolation {
            detail: format!("{kind_label} step requires {predicate} (--field {key}=...)"),
        }),
    }
}

fn optional_i64(fields: &BTreeMap<String, String>, key: &str) -> Result<Option<i64>, HandlerError> {
    let Some(raw) = fields.get(key) else {
        return Ok(None);
    };
    let parsed = raw
        .parse::<i64>()
        .map_err(|e| HandlerError::InvalidArgument {
            field: format!("fields.{key}"),
            detail: format!("{key} must parse as an integer: {e}"),
        })?;
    Ok(Some(parsed))
}

fn optional_bool(
    fields: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<bool>, HandlerError> {
    let Some(raw) = fields.get(key) else {
        return Ok(None);
    };
    match raw.as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => Err(HandlerError::InvalidArgument {
            field: format!("fields.{key}"),
            detail: format!("{key} must be true/false; got {raw:?}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn shell_command_accepts_minimal_command() {
        let fields = map(&[("command", "dec init")]);
        let out = build_step_fields(StepKind::ShellCommand, &fields).expect("ok");
        match out {
            StepFields::ShellCommand { command, .. } => assert_eq!(command, "dec init"),
            other => panic!("expected ShellCommand, got {other:?}"),
        }
    }

    #[test]
    fn shell_command_without_command_is_schema_violation() {
        let fields = map(&[("expect-exit-code", "0")]);
        let err = build_step_fields(StepKind::ShellCommand, &fields).expect_err("must fail");
        match err {
            HandlerError::SchemaViolation { detail } => {
                assert!(detail.contains("dec:command"), "detail: {detail}");
            }
            other => panic!("expected SchemaViolation, got {other:?}"),
        }
    }

    #[test]
    fn unknown_key_for_kind_is_invalid_argument() {
        let fields = map(&[("command", "ls"), ("rocket", "go")]);
        let err = build_step_fields(StepKind::ShellCommand, &fields).expect_err("must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => {
                assert_eq!(field, "fields.rocket");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn sparql_assertion_requires_both_target_and_query() {
        let only_target = map(&[("target", ".dec/store")]);
        let err =
            build_step_fields(StepKind::SparqlAssertion, &only_target).expect_err("must fail");
        match err {
            HandlerError::SchemaViolation { detail } => {
                assert!(detail.contains("dec:query"), "detail: {detail}");
            }
            other => panic!("expected SchemaViolation, got {other:?}"),
        }
    }

    #[test]
    fn capture_requires_bind_as() {
        let fields = map(&[]);
        let err = build_step_fields(StepKind::Capture, &fields).expect_err("must fail");
        match err {
            HandlerError::SchemaViolation { detail } => {
                assert!(detail.contains("dec:bindAs"), "detail: {detail}");
            }
            other => panic!("expected SchemaViolation, got {other:?}"),
        }
    }

    #[test]
    fn shell_command_preserves_dollar_brace_placeholder() {
        let fields = map(&[("command", "dec verify ${prior_capture}")]);
        let out = build_step_fields(StepKind::ShellCommand, &fields).expect("ok");
        match out {
            StepFields::ShellCommand { command, .. } => {
                assert!(command.contains("${prior_capture}"));
            }
            other => panic!("expected ShellCommand, got {other:?}"),
        }
    }
}
