//! Per-stream config for the code-change-committed auto-dispatch (FT-100).
//!
//! ```toml
//! [verify_graph_runner.on_code_change]
//! enabled = true
//! envs = ["*"]
//! parallelism = 1
//! fan_out = "per_env"
//! dedup_ttl_seconds = 60
//! ```

use serde::{Deserialize, Serialize};

/// Default dedup TTL (60 s) per FT-100 §Inputs.
pub const DEFAULT_DEDUP_TTL_SECONDS: u64 = 60;

/// Sentinel for "every env with a covering graph".
pub const ENV_WILDCARD: &str = "*";

/// Resolved per-stream config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeChangeCommittedDispatchConfig {
    /// Master switch.
    pub enabled: bool,
    /// Envs to verify across. `["*"]` means "every env with a covering graph".
    pub envs: Vec<String>,
    /// Sequential per v1.
    pub parallelism: u32,
    /// `per_env` or `per_graph` — v1 always emits per `(graph, env)`.
    pub fan_out: String,
    /// Dedup TTL in seconds.
    pub dedup_ttl_seconds: u64,
    /// Skip aggregate Feedback emission (per-step feedback still emitted by runner).
    #[serde(default)]
    pub feedback_routes_suppress: bool,
}

impl Default for CodeChangeCommittedDispatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            envs: vec![ENV_WILDCARD.to_string()],
            parallelism: 1,
            fan_out: "per_env".to_string(),
            dedup_ttl_seconds: DEFAULT_DEDUP_TTL_SECONDS,
            feedback_routes_suppress: false,
        }
    }
}

impl CodeChangeCommittedDispatchConfig {
    /// True iff envs is the wildcard.
    #[must_use]
    pub fn envs_use_wildcard(&self) -> bool {
        self.envs.iter().any(|e| e == ENV_WILDCARD)
    }
}

/// Parse `[verify_graph_runner.on_code_change]` from a TOML body.
#[must_use]
pub fn parse_from_str(body: &str) -> Option<CodeChangeCommittedDispatchConfig> {
    let mut in_section = false;
    let mut cfg = CodeChangeCommittedDispatchConfig::default();
    let mut saw_any = false;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_section = section.trim() == "verify_graph_runner.on_code_change";
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

/// Load from `<workdir>/.dec/config.toml`; fall back to default.
#[must_use]
pub fn load_from_workdir(workdir: &std::path::Path) -> CodeChangeCommittedDispatchConfig {
    let path = workdir.join(".dec").join("config.toml");
    if let Ok(body) = std::fs::read_to_string(&path) {
        if let Some(cfg) = parse_from_str(&body) {
            return cfg;
        }
    }
    CodeChangeCommittedDispatchConfig::default()
}

fn apply_kv(cfg: &mut CodeChangeCommittedDispatchConfig, key: &str, value: &str) -> bool {
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
        "parallelism" => {
            if let Ok(n) = value.parse::<u32>() {
                cfg.parallelism = n;
                return true;
            }
        }
        "fan_out" => {
            if let Some(s) = parse_string(value) {
                cfg.fan_out = s;
                return true;
            }
        }
        "dedup_ttl_seconds" => {
            if let Ok(n) = value.parse::<u64>() {
                cfg.dedup_ttl_seconds = n;
                return true;
            }
        }
        "feedback_routes" => {
            if let Some(s) = parse_string(value) {
                cfg.feedback_routes_suppress = s == "suppress";
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

fn parse_string(value: &str) -> Option<String> {
    let v = value
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'')
        .trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
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
    fn default_is_enabled_wildcard_per_env() {
        let d = CodeChangeCommittedDispatchConfig::default();
        assert!(d.enabled);
        assert!(d.envs_use_wildcard());
        assert_eq!(d.dedup_ttl_seconds, DEFAULT_DEDUP_TTL_SECONDS);
        assert_eq!(d.fan_out, "per_env");
    }

    #[test]
    fn parser_round_trip() {
        let body = "[verify_graph_runner.on_code_change]\n\
                    enabled = false\n\
                    dedup_ttl_seconds = 5\n\
                    feedback_routes = \"suppress\"\n";
        let cfg = parse_from_str(body).expect("parsed");
        assert!(!cfg.enabled);
        assert_eq!(cfg.dedup_ttl_seconds, 5);
        assert!(cfg.feedback_routes_suppress);
    }
}
