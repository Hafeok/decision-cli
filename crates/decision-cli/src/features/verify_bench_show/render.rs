//! CLI rendering helpers for `dec verify bench show` (FT-040).
//!
//! Two output formats:
//!   * `text` — multi-line human render: id, bench-type, safety-class,
//!     endpoint (if any), allowed-ops list, setup, teardown, each
//!     section indented for legibility. A trailing footer shows the
//!     on-disk path.
//!   * `json` — the [`EnvDocument`] as serde JSON. Optional fields are
//!     omitted (not `null`) so the parity test (AC #4) can byte-compare
//!     CLI and MCP outputs.

use super::BenchShowResponse;

/// Render the response as a multi-line text block.
#[must_use]
pub fn render_text(resp: &BenchShowResponse) -> String {
    let bench = &resp.bench;
    let mut out = String::new();
    out.push_str("VerificationBench\n");
    out.push_str(&format!("  id:           {}\n", bench.id));
    out.push_str(&format!("  bench-type:     {}\n", bench.bench_type));
    out.push_str(&format!("  safety-class: {}\n", bench.safety_class));
    if let Some(ep) = &bench.endpoint {
        out.push_str(&format!("  endpoint:     {ep}\n"));
    }
    if let Some(fs) = &bench.fixture_source {
        out.push_str(&format!("  fixture:      {fs}\n"));
    }
    push_allowed_ops(&mut out, &bench.allowed_ops);
    push_optional_block(&mut out, "setup", bench.setup.as_deref());
    push_optional_block(&mut out, "teardown", bench.teardown.as_deref());
    out.push('\n');
    out.push_str(&format!("Path: {}\n", resp.path.display()));
    out
}

/// Append the `allowed-ops:` section. Empty lists render as `(none)`
/// to keep the block visible in the human render.
fn push_allowed_ops(out: &mut String, ops: &[String]) {
    out.push_str("  allowed-ops:\n");
    if ops.is_empty() {
        out.push_str("    (none)\n");
        return;
    }
    for op in ops {
        out.push_str(&format!("    - {op}\n"));
    }
}

/// Append a `<label>:` section with each body line indented four
/// spaces. Absent bodies render nothing (the section is dropped).
fn push_optional_block(out: &mut String, label: &str, body: Option<&str>) {
    let Some(body) = body else {
        return;
    };
    out.push_str(&format!("  {label}:\n"));
    for line in body.lines() {
        out.push_str(&format!("    {line}\n"));
    }
}

/// Render the bench document as a single JSON object. Optional fields
/// are omitted when absent, matching the MCP envelope's `bench` field.
#[must_use]
pub fn render_json(resp: &BenchShowResponse) -> String {
    serde_json::to_string_pretty(&resp.bench).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::verify_bench_show::EnvDocument;
    use serde_json::Value;
    use std::path::PathBuf;

    fn sample() -> BenchShowResponse {
        BenchShowResponse {
            bench: EnvDocument {
                id: "BNCH-001-ephemeral-cli".to_string(),
                bench_type: "ephemeral-tempdir".to_string(),
                safety_class: "isolated".to_string(),
                endpoint: None,
                allowed_ops: vec![
                    "shell".to_string(),
                    "filesystem".to_string(),
                    "sparql-local".to_string(),
                ],
                setup: Some("mkdir -p \"$TMPDIR\" && cd \"$TMPDIR\"".to_string()),
                teardown: Some("rm -rf \"$TMPDIR\"".to_string()),
                fixture_source: None,
            },
            path: PathBuf::from("/tmp/.dec/verify/bench/BNCH-001-ephemeral-cli.ttl"),
        }
    }

    #[test]
    fn text_renders_every_property() {
        let resp = sample();
        let s = render_text(&resp);
        assert!(s.contains("BNCH-001-ephemeral-cli"));
        assert!(s.contains("ephemeral-tempdir"));
        assert!(s.contains("isolated"));
        assert!(s.contains("shell"));
        assert!(s.contains("filesystem"));
        assert!(s.contains("sparql-local"));
        assert!(s.contains("mkdir"));
        assert!(s.contains("rm -rf"));
        assert!(s.contains("Path:"));
        assert!(s.contains("BNCH-001-ephemeral-cli.ttl"));
    }

    #[test]
    fn text_omits_endpoint_when_absent() {
        let resp = sample();
        let s = render_text(&resp);
        assert!(!s.contains("endpoint:"));
    }

    #[test]
    fn text_includes_endpoint_when_present() {
        let mut resp = sample();
        resp.bench.endpoint = Some("https://example.com".to_string());
        let s = render_text(&resp);
        assert!(s.contains("endpoint:"));
        assert!(s.contains("https://example.com"));
    }

    #[test]
    fn json_omits_optional_when_absent() {
        let mut resp = sample();
        resp.bench.endpoint = None;
        resp.bench.setup = None;
        resp.bench.teardown = None;
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert!(v.is_object());
        assert_eq!(v["id"], "BNCH-001-ephemeral-cli");
        assert!(v.get("endpoint").is_none());
        assert!(v.get("setup").is_none());
        assert!(v.get("teardown").is_none());
    }

    #[test]
    fn json_includes_optional_when_present() {
        let mut resp = sample();
        resp.bench.endpoint = Some("https://example.com".to_string());
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert_eq!(v["endpoint"], "https://example.com");
    }

    /// FT-053: text render shows `fixture:` row when set.
    #[test]
    fn text_includes_fixture_when_present() {
        let mut resp = sample();
        resp.bench.fixture_source = Some("tests/fixtures/demo".to_string());
        let s = render_text(&resp);
        assert!(s.contains("fixture:"));
        assert!(s.contains("tests/fixtures/demo"));
    }

    /// FT-053: text render omits `fixture:` row when unset.
    #[test]
    fn text_omits_fixture_when_absent() {
        let resp = sample();
        let s = render_text(&resp);
        assert!(!s.contains("fixture:"));
    }

    /// FT-053: JSON render includes `fixture_source` field when set.
    #[test]
    fn json_includes_fixture_source_when_present() {
        let mut resp = sample();
        resp.bench.fixture_source = Some("tests/fixtures/demo".to_string());
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert_eq!(v["fixture_source"], "tests/fixtures/demo");
    }
}
