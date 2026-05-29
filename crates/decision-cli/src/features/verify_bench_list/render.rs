//! CLI rendering helpers for `dec verify bench list` (FT-039).
//!
//! Table rendering trims long fields to keep the table readable on a
//! typical 120-column terminal. JSON rendering uses the serde encoding
//! the response already carries — same encoding the MCP surface returns,
//! so the parity TC (AC #7) compares element-for-element.

#[cfg(test)]
use serde_json::Value;

use super::{BenchListResponse, EnvSummary};

const ALLOWED_OPS_WIDTH: usize = 32;
const ENDPOINT_WIDTH: usize = 28;

/// Pretty-print the response as a single-line-per-bench table. Returns a
/// stand-alone string (newline-terminated when non-empty) so the
/// caller can `print!` it directly. Empty responses print the
/// `"no benches yet"` advisory FT-039 specifies (renamed from "no environments yet" by FT-112).
#[must_use]
pub fn render_table(resp: &BenchListResponse) -> String {
    if resp.envs.is_empty() {
        return "no benches yet\n".to_string();
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
    for bench in &resp.envs {
        out.push_str(&render_row(bench));
    }
    out
}

fn render_row(bench: &EnvSummary) -> String {
    let endpoint = bench.endpoint.as_deref().unwrap_or("(none)");
    // For corrupt rows (TC-096), replace the allowed-ops column with a
    // bracketed error marker so the operator can spot the failure mode
    // without consulting the JSON output. The marker carries the
    // structured kind (e.g. `MultipleAllowedOpsHeads`) and, when
    // applicable, the count.
    let ops_display: String = match &bench.error {
        Some(err) => {
            let detail = match err.count {
                Some(n) => format!("<corrupt: {kind}: {n}>", kind = err.kind),
                None => format!("<corrupt: {kind}>", kind = err.kind),
            };
            detail
        }
        None => bench.allowed_ops.join(","),
    };
    format!(
        "{id:<22}  {ty:<22}  {sc:<22}  {ep:<width_ep$}  {ops}\n",
        id = truncate(&bench.id, 22),
        ty = truncate(&bench.bench_type, 22),
        sc = truncate(&bench.safety_class, 22),
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

/// Render the response as a JSON array of bench summaries (one per row).
///
/// The MCP response wraps the same array in `{"envs": [...]}` — TC-061
/// AC #7 compares the CLI JSON output against `response["envs"]`.
#[must_use]
pub fn render_json(resp: &BenchListResponse) -> String {
    serde_json::to_string_pretty(&resp.envs).unwrap_or_else(|_| "[]".to_string())
}

/// Render one-line stderr warnings for corrupt rows (TC-096 AC #5). Each
/// line names the offending bench id and the failure mode; the operator
/// can grep the broken id without re-parsing JSON. Returns an empty
/// string when no rows are corrupt, so callers can `eprint!` it
/// unconditionally without producing trailing blank output.
#[must_use]
pub fn render_stderr_warnings(resp: &BenchListResponse) -> String {
    let mut out = String::new();
    for bench in &resp.envs {
        if let Some(err) = &bench.error {
            out.push_str(&format!(
                "warning: bench {id} is corrupt — {detail} (run `dec verify bench show {id}` to triage)\n",
                id = bench.id,
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
            id: "BNCH-001-ephemeral-cli".to_string(),
            bench_type: "ephemeral-tempdir".to_string(),
            safety_class: "isolated".to_string(),
            endpoint: None,
            allowed_ops: vec!["shell".to_string(), "filesystem".to_string()],
            setup: None,
            teardown: None,
            fixture_source: None,
            error: None,
        }
    }

    /// FT-053: list JSON includes `fixture_source` when set.
    #[test]
    fn json_includes_fixture_source_when_set() {
        let mut bench = sample();
        bench.fixture_source = Some("tests/fixtures/demo".to_string());
        let resp = BenchListResponse { envs: vec![bench] };
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        let arr = v.as_array().expect("array");
        assert_eq!(arr[0]["fixture_source"], "tests/fixtures/demo");
    }

    /// FT-053: list JSON omits `fixture_source` when unset.
    #[test]
    fn json_omits_fixture_source_when_absent() {
        let resp = BenchListResponse {
            envs: vec![sample()],
        };
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        let arr = v.as_array().expect("array");
        assert!(arr[0].get("fixture_source").is_none());
    }

    #[test]
    fn table_renders_empty_with_advisory() {
        let resp = BenchListResponse { envs: vec![] };
        let s = render_table(&resp);
        assert!(s.contains("no benches yet"));
    }

    #[test]
    fn table_renders_rows_with_headers() {
        let resp = BenchListResponse {
            envs: vec![sample()],
        };
        let s = render_table(&resp);
        assert!(s.contains("ID"));
        assert!(s.contains("BNCH-001-ephemeral-cli"));
        assert!(s.contains("ephemeral-tempdir"));
        assert!(s.contains("isolated"));
    }

    #[test]
    fn json_renders_array_of_envs() {
        let resp = BenchListResponse {
            envs: vec![sample()],
        };
        let s = render_json(&resp);
        // Parse it back so we don't depend on whitespace formatting.
        let v: Value = serde_json::from_str(&s).expect("json");
        assert!(v.is_array());
        let arr = v.as_array().expect("array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "BNCH-001-ephemeral-cli");
        // endpoint, setup, teardown must be omitted (None).
        assert!(arr[0].get("endpoint").is_none());
        assert!(arr[0].get("setup").is_none());
        assert!(arr[0].get("teardown").is_none());
    }

    #[test]
    fn json_includes_optional_fields_when_set() {
        let mut bench = sample();
        bench.endpoint = Some("https://example.com".to_string());
        bench.setup = Some("echo hi".to_string());
        let resp = BenchListResponse { envs: vec![bench] };
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
        let mut bench = sample();
        bench.id = "BNCH-BROKEN".to_string();
        bench.allowed_ops.clear();
        bench.error = Some(super::super::EnvRowError::multiple_allowed_ops_heads(2));
        let resp = BenchListResponse { envs: vec![bench] };
        let s = render_table(&resp);
        // The ALLOWED_OPS column is bounded; the marker's prefix
        // (`<corrupt:`) survives truncation, the long kind name
        // (`MultipleAllowedOpsHeads`) may be elided. The TC-096
        // invariant is that the offending bench id and an error-shape
        // keyword are both visible.
        assert!(s.contains("BNCH-BROKEN"));
        assert!(
            s.contains("corrupt"),
            "table missing corruption marker: {s}"
        );
    }

    #[test]
    fn json_emits_typed_error_field_on_corrupt_row() {
        let mut bench = sample();
        bench.id = "BNCH-BROKEN".to_string();
        bench.allowed_ops.clear();
        bench.error = Some(super::super::EnvRowError::multiple_allowed_ops_heads(2));
        let resp = BenchListResponse { envs: vec![bench] };
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
        let mut bench = sample();
        bench.id = "BNCH-BROKEN".to_string();
        bench.allowed_ops.clear();
        bench.error = Some(super::super::EnvRowError::multiple_allowed_ops_heads(3));
        let resp = BenchListResponse { envs: vec![bench] };
        let w = render_stderr_warnings(&resp);
        assert!(w.contains("BNCH-BROKEN"), "missing id in warning: {w}");
        assert!(
            w.contains("dec verify bench show"),
            "missing triage hint: {w}"
        );
    }

    #[test]
    fn stderr_warnings_empty_for_healthy_envs() {
        let resp = BenchListResponse {
            envs: vec![sample()],
        };
        assert_eq!(render_stderr_warnings(&resp), "");
    }
}
