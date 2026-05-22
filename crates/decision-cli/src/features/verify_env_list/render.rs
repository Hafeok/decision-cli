//! CLI rendering helpers for `dec verify env list` (FT-039).
//!
//! Table rendering trims long fields to keep the table readable on a
//! typical 120-column terminal. JSON rendering uses the serde encoding
//! the response already carries — same encoding the MCP surface returns,
//! so the parity TC (AC #7) compares element-for-element.

#[cfg(test)]
use serde_json::Value;

use super::{EnvListResponse, EnvSummary};

const ALLOWED_OPS_WIDTH: usize = 32;
const ENDPOINT_WIDTH: usize = 28;

/// Pretty-print the response as a single-line-per-env table. Returns a
/// stand-alone string (newline-terminated when non-empty) so the
/// caller can `print!` it directly. Empty responses print the
/// `"no environments yet"` advisory FT-039 specifies.
#[must_use]
pub fn render_table(resp: &EnvListResponse) -> String {
    if resp.envs.is_empty() {
        return "no environments yet\n".to_string();
    }
    let mut out = String::new();
    let header = format!(
        "{id:<22}  {ty:<22}  {sc:<22}  {ep:<width_ep$}  {ops}\n",
        id = "ID",
        ty = "TYPE",
        sc = "SAFETY-CLASS",
        ep = "ENDPOINT",
        ops = "ALLOWED-OPS",
        width_ep = ENDPOINT_WIDTH,
    );
    out.push_str(&header);
    out.push_str(&"-".repeat(header.trim_end().len()));
    out.push('\n');
    for env in &resp.envs {
        out.push_str(&render_row(env));
    }
    out
}

fn render_row(env: &EnvSummary) -> String {
    let endpoint = env.endpoint.as_deref().unwrap_or("(none)");
    // For corrupt rows (TC-096), replace the allowed-ops column with a
    // bracketed error marker so the operator can spot the failure mode
    // without consulting the JSON output. The marker carries the
    // structured kind (e.g. `MultipleAllowedOpsHeads`) and, when
    // applicable, the count.
    let ops_display: String = match &env.error {
        Some(err) => {
            let detail = match err.count {
                Some(n) => format!("<corrupt: {kind}: {n}>", kind = err.kind),
                None => format!("<corrupt: {kind}>", kind = err.kind),
            };
            detail
        }
        None => env.allowed_ops.join(","),
    };
    format!(
        "{id:<22}  {ty:<22}  {sc:<22}  {ep:<width_ep$}  {ops}\n",
        id = truncate(&env.id, 22),
        ty = truncate(&env.env_type, 22),
        sc = truncate(&env.safety_class, 22),
        ep = truncate(endpoint, ENDPOINT_WIDTH),
        ops = truncate(&ops_display, ALLOWED_OPS_WIDTH),
        width_ep = ENDPOINT_WIDTH,
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Render the response as a JSON array of env summaries (one per row).
///
/// The MCP response wraps the same array in `{"envs": [...]}` — TC-061
/// AC #7 compares the CLI JSON output against `response["envs"]`.
#[must_use]
pub fn render_json(resp: &EnvListResponse) -> String {
    serde_json::to_string_pretty(&resp.envs).unwrap_or_else(|_| "[]".to_string())
}

/// Render one-line stderr warnings for corrupt rows (TC-096 AC #5). Each
/// line names the offending env id and the failure mode; the operator
/// can grep the broken id without re-parsing JSON. Returns an empty
/// string when no rows are corrupt, so callers can `eprint!` it
/// unconditionally without producing trailing blank output.
#[must_use]
pub fn render_stderr_warnings(resp: &EnvListResponse) -> String {
    let mut out = String::new();
    for env in &resp.envs {
        if let Some(err) = &env.error {
            out.push_str(&format!(
                "warning: env {id} is corrupt — {detail} (run `dec verify env show {id}` to triage)\n",
                id = env.id,
                detail = err.detail,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EnvSummary {
        EnvSummary {
            id: "ENV-001-ephemeral-cli".to_string(),
            env_type: "ephemeral-tempdir".to_string(),
            safety_class: "isolated".to_string(),
            endpoint: None,
            allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
            setup: None,
            teardown: None,
            error: None,
        }
    }

    #[test]
    fn table_renders_empty_with_advisory() {
        let resp = EnvListResponse { envs: vec![] };
        let s = render_table(&resp);
        assert!(s.contains("no environments yet"));
    }

    #[test]
    fn table_renders_rows_with_headers() {
        let resp = EnvListResponse {
            envs: vec![sample()],
        };
        let s = render_table(&resp);
        assert!(s.contains("ID"));
        assert!(s.contains("ENV-001-ephemeral-cli"));
        assert!(s.contains("ephemeral-tempdir"));
        assert!(s.contains("isolated"));
    }

    #[test]
    fn json_renders_array_of_envs() {
        let resp = EnvListResponse {
            envs: vec![sample()],
        };
        let s = render_json(&resp);
        // Parse it back so we don't depend on whitespace formatting.
        let v: Value = serde_json::from_str(&s).expect("json");
        assert!(v.is_array());
        let arr = v.as_array().expect("array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "ENV-001-ephemeral-cli");
        // endpoint, setup, teardown must be omitted (None).
        assert!(arr[0].get("endpoint").is_none());
        assert!(arr[0].get("setup").is_none());
        assert!(arr[0].get("teardown").is_none());
    }

    #[test]
    fn json_includes_optional_fields_when_set() {
        let mut env = sample();
        env.endpoint = Some("https://example.com".to_string());
        env.setup = Some("echo hi".to_string());
        let resp = EnvListResponse { envs: vec![env] };
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        let arr = v.as_array().expect("array");
        assert_eq!(arr[0]["endpoint"], "https://example.com");
        assert_eq!(arr[0]["setup"], "echo hi");
    }

    #[test]
    fn truncate_preserves_short_strings() {
        assert_eq!(truncate("abc", 10), "abc");
        let long = truncate("abcdefghij", 5);
        assert!(long.ends_with('…'));
        assert!(long.chars().count() == 5);
    }

    #[test]
    fn table_marks_corrupt_row_with_marker() {
        let mut env = sample();
        env.id = "ENV-BROKEN".to_string();
        env.allowed_ops.clear();
        env.error = Some(super::super::EnvRowError::multiple_allowed_ops_heads(2));
        let resp = EnvListResponse { envs: vec![env] };
        let s = render_table(&resp);
        // The ALLOWED_OPS column is bounded; the marker's prefix
        // (`<corrupt:`) survives truncation, the long kind name
        // (`MultipleAllowedOpsHeads`) may be elided. The TC-096
        // invariant is that the offending env id and an error-shape
        // keyword are both visible.
        assert!(s.contains("ENV-BROKEN"));
        assert!(
            s.contains("corrupt"),
            "table missing corruption marker: {s}"
        );
    }

    #[test]
    fn json_emits_typed_error_field_on_corrupt_row() {
        let mut env = sample();
        env.id = "ENV-BROKEN".to_string();
        env.allowed_ops.clear();
        env.error = Some(super::super::EnvRowError::multiple_allowed_ops_heads(2));
        let resp = EnvListResponse { envs: vec![env] };
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        let arr = v.as_array().expect("array");
        let err = &arr[0]["error"];
        assert_eq!(err["kind"], "MultipleAllowedOpsHeads");
        assert_eq!(err["count"], 2);
        assert!(err["detail"].is_string());
    }

    #[test]
    fn stderr_warnings_lists_each_broken_env() {
        let mut env = sample();
        env.id = "ENV-BROKEN".to_string();
        env.allowed_ops.clear();
        env.error = Some(super::super::EnvRowError::multiple_allowed_ops_heads(3));
        let resp = EnvListResponse { envs: vec![env] };
        let w = render_stderr_warnings(&resp);
        assert!(w.contains("ENV-BROKEN"), "missing id in warning: {w}");
        assert!(
            w.contains("dec verify env show"),
            "missing triage hint: {w}"
        );
    }

    #[test]
    fn stderr_warnings_empty_for_healthy_envs() {
        let resp = EnvListResponse {
            envs: vec![sample()],
        };
        assert_eq!(render_stderr_warnings(&resp), "");
    }
}
