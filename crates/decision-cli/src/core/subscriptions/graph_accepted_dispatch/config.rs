//! Per-stream config for the graph-accepted auto-dispatch subscription (FT-100).
//!
//! Authored under `.dec/config.toml`:
//!
//! ```toml
//! [verify_graph_runner.on_graph_accepted]
//! enabled = true                       # default true
//! envs = ["*"]                         # "*" = the graph's declared env
//! dedup_ttl_seconds = 300              # 5 min dedup window
//! ```

use serde::{Deserialize, Serialize};

/// Default dedup TTL (5 min) per FT-100 §State.
pub const DEFAULT_DEDUP_TTL_SECONDS: u64 = 300;

/// Sentinel for "the graph's own declared env".
pub const ENV_WILDCARD: &str = "*";

/// Resolved per-stream config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphAcceptedDispatchConfig {
    /// Master switch. When `false`, the handler is a no-op.
    pub enabled: bool,
    /// Envs to dispatch against. `["*"]` means "the graph's declared env";
    /// any other list restricts to that subset (matching by short id).
    pub envs: Vec<String>,
    /// Dedup TTL in seconds. `0` disables dedup.
    pub dedup_ttl_seconds: u64,
}

impl Default for GraphAcceptedDispatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            envs: vec![ENV_WILDCARD.to_string()],
            dedup_ttl_seconds: DEFAULT_DEDUP_TTL_SECONDS,
        }
    }
}

impl GraphAcceptedDispatchConfig {
    /// Returns true when the env list is the wildcard.
    #[must_use]
    pub fn envs_use_wildcard(&self) -> bool {
        self.envs.iter().any(|e| e == ENV_WILDCARD)
    }
}

/// Minimal parser for the `[verify_graph_runner.on_graph_accepted]` table
/// embedded in `.dec/config.toml`. Returns `None` when the section is
/// absent — callers fall back to [`GraphAcceptedDispatchConfig::default`].
#[must_use]
pub fn parse_from_str(body: &str) -> Option<GraphAcceptedDispatchConfig> {
    let mut in_section = false;
    let mut cfg = GraphAcceptedDispatchConfig::default();
    let mut saw_any = false;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_section = section.trim() == "verify_graph_runner.on_graph_accepted";
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_end_matches('#').trim();
        if apply_kv(&mut cfg, key, value) {
            saw_any = true;
        }
    }
    if saw_any {
        Some(cfg)
    } else {
        None
    }
}

/// Load config from disk (`<workdir>/.dec/config.toml`); fall back to
/// default when missing or section absent.
#[must_use]
pub fn load_from_workdir(workdir: &std::path::Path) -> GraphAcceptedDispatchConfig {
    let path = workdir.join(".dec").join("config.toml");
    if let Ok(body) = std::fs::read_to_string(&path) {
        if let Some(cfg) = parse_from_str(&body) {
            return cfg;
        }
    }
    GraphAcceptedDispatchConfig::default()
}

fn apply_kv(cfg: &mut GraphAcceptedDispatchConfig, key: &str, value: &str) -> bool {
    match key {
        "enabled" => {
            if let Some(b) = parse_bool(value) {
                cfg.enabled = b;
                return true;
            }
        }
        "envs" => {
            if let Some(list) = parse_string_array(value) {
                cfg.envs = list;
                return true;
            }
        }
        "dedup_ttl_seconds" => {
            if let Ok(n) = value.parse::<u64>() {
                cfg.dedup_ttl_seconds = n;
                return true;
            }
        }
        _ => {}
    }
    false
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_string_array(value: &str) -> Option<Vec<String>> {
    let inner = value.strip_prefix('[')?.strip_suffix(']')?;
    let mut out = Vec::new();
    for raw in inner.split(',') {
        let v = raw
            .trim()
            .trim_matches(|c: char| c == '"' || c == '\'')
            .trim();
        if !v.is_empty() {
            out.push(v.to_string());
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_enabled_wildcard() {
        let d = GraphAcceptedDispatchConfig::default();
        assert!(d.enabled);
        assert!(d.envs_use_wildcard());
        assert_eq!(d.dedup_ttl_seconds, DEFAULT_DEDUP_TTL_SECONDS);
    }

    #[test]
    fn parser_round_trip() {
        let body = "[verify_graph_runner.on_graph_accepted]\n\
                    enabled = false\n\
                    envs = [\"ENV-001\"]\n\
                    dedup_ttl_seconds = 5\n";
        let cfg = parse_from_str(body).expect("parsed");
        assert!(!cfg.enabled);
        assert_eq!(cfg.envs, vec!["ENV-001".to_string()]);
        assert_eq!(cfg.dedup_ttl_seconds, 5);
    }

    #[test]
    fn parser_skips_other_sections() {
        let body = "[other]\nenabled = false\n\
                    [verify_graph_runner.on_graph_accepted]\nenabled = true\n";
        let cfg = parse_from_str(body).expect("parsed");
        assert!(cfg.enabled);
    }
}
