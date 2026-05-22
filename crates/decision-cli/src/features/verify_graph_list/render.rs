//! CLI rendering helpers for `dec verify graph list` (FT-042).
//!
//! Table rendering trims long fields to keep the table readable on a
//! typical 120-column terminal. JSON rendering uses the serde encoding
//! the response already carries — same encoding the MCP surface returns,
//! so the parity TC (AC #7) compares element-for-element.

#[cfg(test)]
use serde_json::Value;

use super::{GraphListResponse, GraphSummary};

const VERIFIES_WIDTH: usize = 20;
const ENVIRONMENT_WIDTH: usize = 30;
const ID_WIDTH: usize = 24;

/// Pretty-print the response as a single-line-per-graph table. Returns
/// a stand-alone string (newline-terminated when non-empty) so the
/// caller can `print!` it directly. Empty responses print the
/// `"no verification graphs yet"` advisory FT-042 specifies.
#[must_use]
pub fn render_table(resp: &GraphListResponse) -> String {
    if resp.graphs.is_empty() {
        return "no verification graphs yet\n".to_string();
    }
    let mut out = String::new();
    let header = format!(
        "{id:<width_id$}  {ver:<width_ver$}  {env:<width_env$}  {steps}\n",
        id = "ID",
        ver = "VERIFIES",
        env = "ENVIRONMENT",
        steps = "STEPS",
        width_id = ID_WIDTH,
        width_ver = VERIFIES_WIDTH,
        width_env = ENVIRONMENT_WIDTH,
    );
    out.push_str(&header);
    out.push_str(&"-".repeat(header.trim_end().len()));
    out.push('\n');
    for graph in &resp.graphs {
        out.push_str(&render_row(graph));
    }
    out
}

fn render_row(graph: &GraphSummary) -> String {
    format!(
        "{id:<width_id$}  {ver:<width_ver$}  {env:<width_env$}  {steps}\n",
        id = truncate(&graph.id, ID_WIDTH),
        ver = truncate(&graph.verifies, VERIFIES_WIDTH),
        env = truncate(&graph.environment, ENVIRONMENT_WIDTH),
        steps = graph.step_count,
        width_id = ID_WIDTH,
        width_ver = VERIFIES_WIDTH,
        width_env = ENVIRONMENT_WIDTH,
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

/// Render the response as a JSON array of graph summaries (one per row).
///
/// The MCP response wraps the same array in `{"graphs": [...]}` — TC-064
/// AC #7 compares the CLI JSON output against `response["graphs"]`.
#[must_use]
pub fn render_json(resp: &GraphListResponse) -> String {
    serde_json::to_string_pretty(&resp.graphs).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GraphSummary {
        GraphSummary {
            id: "VG-001".to_string(),
            verifies: "FT-001".to_string(),
            environment: "ENV-001-ephemeral-cli".to_string(),
            step_count: 3,
        }
    }

    #[test]
    fn table_renders_empty_with_advisory() {
        let resp = GraphListResponse { graphs: vec![] };
        let s = render_table(&resp);
        assert!(s.contains("no verification graphs yet"));
    }

    #[test]
    fn table_renders_rows_with_headers() {
        let resp = GraphListResponse {
            graphs: vec![sample()],
        };
        let s = render_table(&resp);
        assert!(s.contains("ID"));
        assert!(s.contains("VERIFIES"));
        assert!(s.contains("ENVIRONMENT"));
        assert!(s.contains("STEPS"));
        assert!(s.contains("VG-001"));
        assert!(s.contains("FT-001"));
        assert!(s.contains("ENV-001-ephemeral-cli"));
        assert!(s.contains("3"));
    }

    #[test]
    fn json_renders_array_of_graphs() {
        let resp = GraphListResponse {
            graphs: vec![sample()],
        };
        let s = render_json(&resp);
        // Parse it back so we don't depend on whitespace formatting.
        let v: Value = serde_json::from_str(&s).expect("json");
        assert!(v.is_array());
        let arr = v.as_array().expect("array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "VG-001");
        assert_eq!(arr[0]["verifies"], "FT-001");
        assert_eq!(arr[0]["environment"], "ENV-001-ephemeral-cli");
        assert_eq!(arr[0]["step_count"], 3);
    }

    #[test]
    fn json_empty_response_renders_empty_array() {
        let resp = GraphListResponse { graphs: vec![] };
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert!(v.is_array());
        assert_eq!(v.as_array().expect("arr").len(), 0);
    }

    #[test]
    fn truncate_preserves_short_strings() {
        assert_eq!(truncate("abc", 10), "abc");
        let long = truncate("abcdefghij", 5);
        assert!(long.ends_with('…'));
        assert!(long.chars().count() == 5);
    }
}
