//! Per-stream configuration for the verify-graph-author auto-dispatch subscription.
//!
//! Authored under `.dec/config.toml` (FT-050 §Inputs):
//!
//! ```toml
//! [verify_graph_author]
//! auto_dispatch = true
//! envs = ["ENV-001-ephemeral-cli"]
//! dedup_ttl_seconds = 3600
//! ```
//!
//! Slice 2.6 keeps the parser intentionally tiny — no `toml` crate
//! dependency is pulled in yet; the subscription receives a structured
//! [`AutoDispatchConfig`] from the caller (the orchestrator wires this
//! when it reads `.dec/config.toml`). Future slices may add the file
//! parser when other features start consuming the same config.

use serde::{Deserialize, Serialize};

/// The default dedup TTL (1 hour) per FT-050 §State.
pub const DEFAULT_DEDUP_TTL_SECONDS: u64 = 3600;

/// Sentinel value meaning "every env in the catalog whose
/// safety_class is `isolated`" (per FT-050 §Inputs).
pub const ENV_WILDCARD: &str = "*";

/// Resolved per-stream config for the auto-dispatch subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoDispatchConfig {
    /// Master switch. When `false`, the handler is a no-op.
    pub auto_dispatch: bool,
    /// Envs to propose against. `["*"]` means "every isolated env in
    /// the catalog"; an explicit list (`["ENV-001-ephemeral-cli"]`)
    /// scopes the subscription to that subset.
    pub envs: Vec<String>,
    /// Dedup TTL in seconds. `0` disables dedup (testing only — see
    /// TC-084 AC #4).
    pub dedup_ttl_seconds: u64,
}

impl Default for AutoDispatchConfig {
    fn default() -> Self {
        Self {
            auto_dispatch: true,
            envs: vec![ENV_WILDCARD.to_string()],
            dedup_ttl_seconds: DEFAULT_DEDUP_TTL_SECONDS,
        }
    }
}

impl AutoDispatchConfig {
    /// Test helper — explicit env list with default TTL.
    #[must_use]
    pub fn with_envs<I, S>(envs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            auto_dispatch: true,
            envs: envs.into_iter().map(Into::into).collect(),
            dedup_ttl_seconds: DEFAULT_DEDUP_TTL_SECONDS,
        }
    }

    /// Test helper — explicit TTL, no envs.
    #[must_use]
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.dedup_ttl_seconds = ttl_seconds;
        self
    }

    /// True iff the env list explicitly contains the wildcard.
    #[must_use]
    pub fn envs_use_wildcard(&self) -> bool {
        self.envs.iter().any(|e| e == ENV_WILDCARD)
    }
}

/// Minimal TOML parser for `[verify_graph_author]`. Accepts the three
/// keys above; ignores everything else. Returns `None` if the file does
/// not exist or the `[verify_graph_author]` table is absent — callers
/// then fall back to `AutoDispatchConfig::default()` and emit a warning
/// event (per FT-050 §Error handling).
///
/// The parser is line-based and only handles the small subset of TOML
/// the spec actually uses (bools, integers, single-line string arrays).
/// Adopting `toml` as a crate dependency is deferred until other
/// features consume the same config.
#[must_use]
pub fn parse_from_str(body: &str) -> Option<AutoDispatchConfig> {
    let mut in_section = false;
    let mut cfg = AutoDispatchConfig::default();
    let mut saw_any = false;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_section = section.trim() == "verify_graph_author";
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

fn apply_kv(cfg: &mut AutoDispatchConfig, key: &str, value: &str) -> bool {
    match key {
        "auto_dispatch" => {
            if let Some(b) = parse_bool(value) {
                cfg.auto_dispatch = b;
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
    fn default_is_auto_dispatch_on_wildcard() {
        let d = AutoDispatchConfig::default();
        assert!(d.auto_dispatch);
        assert!(d.envs_use_wildcard());
        assert_eq!(d.dedup_ttl_seconds, DEFAULT_DEDUP_TTL_SECONDS);
    }

    #[test]
    fn parser_round_trips_keys() {
        let body = "[verify_graph_author]\n\
                    auto_dispatch = true\n\
                    envs = [\"ENV-001-ephemeral-cli\"]\n\
                    dedup_ttl_seconds = 7200\n";
        let cfg = parse_from_str(body).expect("parsed");
        assert!(cfg.auto_dispatch);
        assert_eq!(cfg.envs, vec!["ENV-001-ephemeral-cli".to_string()]);
        assert_eq!(cfg.dedup_ttl_seconds, 7200);
    }

    #[test]
    fn parser_ignores_other_sections() {
        let body = "[other]\nauto_dispatch = false\n\
                    [verify_graph_author]\nauto_dispatch = false\n";
        let cfg = parse_from_str(body).expect("parsed");
        assert!(!cfg.auto_dispatch);
    }

    #[test]
    fn parser_returns_none_when_section_absent() {
        let body = "[other]\nfoo = 1\n";
        assert!(parse_from_str(body).is_none());
    }

    #[test]
    fn ttl_zero_disables_dedup() {
        let cfg = AutoDispatchConfig::default().with_ttl(0);
        assert_eq!(cfg.dedup_ttl_seconds, 0);
    }
}
