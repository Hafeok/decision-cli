//! Text + JSON renderers for the `dec loop` surfaces.

use serde::{Deserialize, Serialize};

use super::list::LoopListResponse;
use super::show::LoopShowResponse;

/// Output format selector. `Text` is the operator default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Text
    }
}

impl OutputFormat {
    /// Parse the wire value. Unknown formats yield `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[must_use]
pub fn show_response(resp: &LoopShowResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(resp)
            .unwrap_or_else(|_| "{}".to_string())
            + "\n",
        OutputFormat::Text => render_show_text(resp),
    }
}

#[must_use]
pub fn list_response(resp: &LoopListResponse, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(resp)
            .unwrap_or_else(|_| "{}".to_string())
            + "\n",
        OutputFormat::Text => render_list_text(resp),
    }
}

fn render_show_text(resp: &LoopShowResponse) -> String {
    let mut out = String::new();
    if resp.entries.is_empty() {
        out.push_str(&format!("(no feedback for {})\n", resp.feature_id));
        return out;
    }
    out.push_str(&format!("Loop chain for {}:\n", resp.feature_id));
    for entry in &resp.entries {
        let glyph = state_glyph(&entry.state);
        let when = entry
            .routed_at
            .as_deref()
            .unwrap_or("(not yet routed)");
        let evidence = entry.evidence.lines().next().unwrap_or("").trim();
        let evidence_excerpt = if evidence.len() > 200 {
            format!("{}…", &evidence[..200])
        } else {
            evidence.to_string()
        };
        out.push_str(&format!(
            "  {glyph} [{state}] {tc} {when} via {sess} — {fb}\n",
            state = entry.state,
            tc = entry.source_tc_short,
            sess = entry.source_session_short,
            fb = super::resolver::short_for_feedback(&entry.feedback_iri),
        ));
        out.push_str(&format!("      ↳ evidence: {evidence_excerpt}\n"));
        if let Some(addr) = &entry.addressing_artifact_short {
            out.push_str(&format!("      ↳ addressed by: {addr}\n"));
        }
        if let Some(rec) = &entry.receiving_session_short {
            out.push_str(&format!("      ↳ received by: {rec}\n"));
        }
    }
    let open = resp
        .entries
        .iter()
        .filter(|e| matches!(e.state.as_str(), "produced" | "routed" | "received"))
        .count();
    let closed = resp.entries.len() - open;
    out.push_str(&format!("\n  Total: {} ({} open, {} closed)\n", resp.entries.len(), open, closed));
    out
}

fn render_list_text(resp: &LoopListResponse) -> String {
    let mut out = String::new();
    if resp.rows.is_empty() {
        out.push_str("(no features with feedback in the requested state)\n");
    } else {
        out.push_str("FEATURE     OPEN  CLOSED  LAST-EMITTED\n");
        out.push_str("--------------------------------------------------\n");
        for row in &resp.rows {
            let last = row.last_emitted_at.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "{:<11} {:>4}  {:>6}  {}\n",
                row.feature_id, row.open_count, row.closed_count, last,
            ));
        }
    }
    if resp.unscoped_count > 0 {
        out.push_str(&format!(
            "\n({} feedback artifact(s) not scoped to any feature)\n",
            resp.unscoped_count
        ));
    }
    out
}

fn state_glyph(state: &str) -> &'static str {
    match state {
        "produced" => "○",
        "routed" => "◐",
        "received" => "◑",
        "addressed" => "●",
        "closed" => "◉",
        "rejected" => "✗",
        "superseded" => "↻",
        _ => "·",
    }
}
