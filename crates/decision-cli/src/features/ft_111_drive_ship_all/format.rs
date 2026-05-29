//! Output formatters for sweep results (FT-111 §PAT-003).
//!
//! Pure functions over `(rows, tally)` — no I/O, no graph reads.

use super::sweep::{Outcome, Row, Tally};

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Tsv,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Format::Text),
            "tsv" => Ok(Format::Tsv),
            "json" => Ok(Format::Json),
            _ => Err(format!("unknown format '{s}'; expected text, tsv, or json")),
        }
    }
}

/// Render sweep results in the requested format.
pub fn render(rows: &[Row], tally: &Tally, format: Format) -> String {
    match format {
        Format::Text => render_text(rows, tally),
        Format::Tsv => render_tsv(rows, tally),
        Format::Json => render_json(rows, tally),
    }
}

fn render_text(rows: &[Row], tally: &Tally) -> String {
    let mut out = String::new();
    out.push_str("Feature Sweep Results\n");
    out.push_str("=====================\n\n");

    for row in rows {
        let outcome_str = match &row.outcome {
            Outcome::Done => "✓ Done".to_string(),
            Outcome::Stuck { reason } => format!("✗ Stuck: {reason}"),
            Outcome::HitMaxIter => "✗ Max iterations".to_string(),
            Outcome::Timeout { after_secs } => format!("✗ Timeout ({}s)", after_secs),
            Outcome::Error { detail } => format!("✗ Error: {detail}"),
        };
        out.push_str(&format!(
            "{:8} {:6}ms  {:3} iter  {}\n",
            row.feature_id, row.elapsed_ms, row.iterations, outcome_str
        ));
    }

    out.push_str("\nSummary\n");
    out.push_str("-------\n");
    out.push_str(&format!("Done:      {:3}\n", tally.done));
    out.push_str(&format!("Stuck:     {:3}\n", tally.stuck));
    out.push_str(&format!("Max iter:  {:3}\n", tally.max_iter));
    out.push_str(&format!("Timeout:   {:3}\n", tally.timeout));
    out.push_str(&format!("Error:     {:3}\n", tally.error));
    out.push_str(&format!(
        "Total:     {:3}\n",
        tally.done + tally.stuck + tally.max_iter + tally.timeout + tally.error
    ));

    out
}

fn render_tsv(rows: &[Row], _tally: &Tally) -> String {
    let mut out = String::new();
    out.push_str("feature_id\toutcome\titerations\telapsed_ms\n");

    for row in rows {
        let outcome_str = match &row.outcome {
            Outcome::Done => "done".to_string(),
            Outcome::Stuck { reason } => format!("stuck:{}", reason.replace('\t', " ")),
            Outcome::HitMaxIter => "max_iter".to_string(),
            Outcome::Timeout { after_secs } => format!("timeout:{after_secs}s"),
            Outcome::Error { detail } => format!("error:{}", detail.replace('\t', " ")),
        };
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            row.feature_id, outcome_str, row.iterations, row.elapsed_ms
        ));
    }

    out
}

fn render_json(rows: &[Row], tally: &Tally) -> String {
    let obj = serde_json::json!({
        "rows": rows,
        "tally": tally,
    });
    serde_json::to_string_pretty(&obj).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_rows() -> Vec<Row> {
        vec![
            Row {
                feature_id: "FT-1".to_string(),
                outcome: Outcome::Done,
                iterations: 3,
                elapsed_ms: 1200,
            },
            Row {
                feature_id: "FT-2".to_string(),
                outcome: Outcome::Stuck {
                    reason: "implementer stuck".to_string(),
                },
                iterations: 5,
                elapsed_ms: 2500,
            },
            Row {
                feature_id: "FT-3".to_string(),
                outcome: Outcome::HitMaxIter,
                iterations: 6,
                elapsed_ms: 3000,
            },
            Row {
                feature_id: "FT-4".to_string(),
                outcome: Outcome::Timeout { after_secs: 600 },
                iterations: 0,
                elapsed_ms: 600000,
            },
            Row {
                feature_id: "FT-5".to_string(),
                outcome: Outcome::Error {
                    detail: "store unreachable".to_string(),
                },
                iterations: 0,
                elapsed_ms: 50,
            },
        ]
    }

    fn make_test_tally() -> Tally {
        Tally {
            done: 1,
            stuck: 1,
            max_iter: 1,
            timeout: 1,
            error: 1,
        }
    }

    #[test]
    fn text_format_includes_all_rows() {
        let rows = make_test_rows();
        let tally = make_test_tally();
        let output = render_text(&rows, &tally);

        assert!(output.contains("FT-1"));
        assert!(output.contains("FT-2"));
        assert!(output.contains("FT-3"));
        assert!(output.contains("FT-4"));
        assert!(output.contains("FT-5"));
        assert!(output.contains("Done:"));
        assert!(output.contains("Stuck:"));
        // Check that the counts are present somewhere in the output
        assert!(output.matches("  1").count() >= 5); // 5 categories with count 1
    }

    #[test]
    fn tsv_format_parseable() {
        let rows = make_test_rows();
        let tally = make_test_tally();
        let output = render_tsv(&rows, &tally);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "feature_id\toutcome\titerations\telapsed_ms");
        assert!(lines[1].starts_with("FT-1\tdone\t3\t"));
        assert!(lines[2].contains("stuck:"));
    }

    #[test]
    fn json_format_has_stable_shape() {
        let rows = make_test_rows();
        let tally = make_test_tally();
        let output = render_json(&rows, &tally);

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("rows").is_some());
        assert!(parsed.get("tally").is_some());

        let tally_obj = parsed.get("tally").unwrap();
        assert_eq!(tally_obj.get("done").unwrap().as_u64().unwrap(), 1);
        assert_eq!(tally_obj.get("stuck").unwrap().as_u64().unwrap(), 1);
        assert_eq!(tally_obj.get("max_iter").unwrap().as_u64().unwrap(), 1);
        assert_eq!(tally_obj.get("timeout").unwrap().as_u64().unwrap(), 1);
        assert_eq!(tally_obj.get("error").unwrap().as_u64().unwrap(), 1);
    }
}
