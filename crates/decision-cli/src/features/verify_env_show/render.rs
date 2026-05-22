//! CLI rendering helpers for `dec verify env show` (FT-040).
//!
//! Two output formats:
//!   * `text` — multi-line human render: id, env-type, safety-class,
//!     endpoint (if any), allowed-ops list, setup, teardown, each
//!     section indented for legibility. A trailing footer shows the
//!     on-disk path.
//!   * `json` — the [`EnvDocument`] as serde JSON. Optional fields are
//!     omitted (not `null`) so the parity test (AC #4) can byte-compare
//!     CLI and MCP outputs.

use super::EnvShowResponse;

/// Render the response as a multi-line text block.
#[must_use]
pub fn render_text(resp: &EnvShowResponse) -> String {
    let env = &resp.env;
    let mut out = String::new();
    out.push_str("VerificationEnvironment\n");
    out.push_str(&format!("  id:           {}\n", env.id));
    out.push_str(&format!("  env-type:     {}\n", env.env_type));
    out.push_str(&format!("  safety-class: {}\n", env.safety_class));
    if let Some(ep) = &env.endpoint {
        out.push_str(&format!("  endpoint:     {ep}\n"));
    }
    if let Some(fs) = &env.fixture_source {
        out.push_str(&format!("  fixture:      {fs}\n"));
    }
    push_allowed_ops(&mut out, &env.allowed_ops);
    push_optional_block(&mut out, "setup", env.setup.as_deref());
    push_optional_block(&mut out, "teardown", env.teardown.as_deref());
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

/// Render the env document as a single JSON object. Optional fields
/// are omitted when absent, matching the MCP envelope's `env` field.
#[must_use]
pub fn render_json(resp: &EnvShowResponse) -> String {
    serde_json::to_string_pretty(&resp.env).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::verify_env_show::EnvDocument;
    use serde_json::Value;
    use std::path::PathBuf;

    fn sample() -> EnvShowResponse {
        EnvShowResponse {
            env: EnvDocument {
                id: "ENV-001-ephemeral-cli".to_string(),
                env_type: "ephemeral-tempdir".to_string(),
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
            path: PathBuf::from("/tmp/.dec/verify/env/ENV-001-ephemeral-cli.ttl"),
        }
    }

    #[test]
    fn text_renders_every_property() {
        let resp = sample();
        let s = render_text(&resp);
        assert!(s.contains("ENV-001-ephemeral-cli"));
        assert!(s.contains("ephemeral-tempdir"));
        assert!(s.contains("isolated"));
        assert!(s.contains("shell"));
        assert!(s.contains("filesystem"));
        assert!(s.contains("sparql-local"));
        assert!(s.contains("mkdir"));
        assert!(s.contains("rm -rf"));
        assert!(s.contains("Path:"));
        assert!(s.contains("ENV-001-ephemeral-cli.ttl"));
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
        resp.env.endpoint = Some("https://example.com".to_string());
        let s = render_text(&resp);
        assert!(s.contains("endpoint:"));
        assert!(s.contains("https://example.com"));
    }

    #[test]
    fn json_omits_optional_when_absent() {
        let mut resp = sample();
        resp.env.endpoint = None;
        resp.env.setup = None;
        resp.env.teardown = None;
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert!(v.is_object());
        assert_eq!(v["id"], "ENV-001-ephemeral-cli");
        assert!(v.get("endpoint").is_none());
        assert!(v.get("setup").is_none());
        assert!(v.get("teardown").is_none());
    }

    #[test]
    fn json_includes_optional_when_present() {
        let mut resp = sample();
        resp.env.endpoint = Some("https://example.com".to_string());
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert_eq!(v["endpoint"], "https://example.com");
    }

    /// FT-053: text render shows `fixture:` row when set.
    #[test]
    fn text_includes_fixture_when_present() {
        let mut resp = sample();
        resp.env.fixture_source = Some("tests/fixtures/demo".to_string());
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
        resp.env.fixture_source = Some("tests/fixtures/demo".to_string());
        let s = render_json(&resp);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert_eq!(v["fixture_source"], "tests/fixtures/demo");
    }
}
