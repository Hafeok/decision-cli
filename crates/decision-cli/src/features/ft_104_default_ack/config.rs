//! `[features] default-acknowledged-cross-cutting` — config-level list of
//! ADR IDs the repo defaults to acknowledging.
//!
//! Looks for the TOML file in two locations to match both the product-cli
//! discovery order (`<root>/product.toml`) and decision-cli's existing
//! convention (`<root>/.product/config.toml`). An empty list or absent
//! key preserves the pre-FT-104 behavior (every cross-cutting ADR shows
//! up as a per-feature gap unless explicitly linked).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Parsed `[features] default-acknowledged-cross-cutting` block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefaultAcknowledgeConfig {
    /// The set of ADR IDs the repo defaults to acknowledging. Stored as
    /// a BTreeSet so membership tests are O(log n) and ordering is
    /// deterministic for diagnostics.
    pub adrs: BTreeSet<String>,
    /// Path the config was loaded from. `None` when neither
    /// `product.toml` nor `.product/config.toml` exists.
    pub source: Option<PathBuf>,
}

impl DefaultAcknowledgeConfig {
    /// `true` iff `adr_id` is listed in
    /// `default-acknowledged-cross-cutting`.
    #[must_use]
    pub fn acknowledges(&self, adr_id: &str) -> bool {
        self.adrs.contains(adr_id)
    }
}

/// Load the config from disk. Returns a `Default` value when neither
/// candidate file is present, when the file lacks a `[features]` table,
/// or when the key is absent or empty.
///
/// Errors only when the file is present and *malformed* TOML.
#[must_use]
pub fn load_default_acknowledge(workdir: &Path) -> DefaultAcknowledgeConfig {
    for candidate in candidate_paths(workdir) {
        if !candidate.exists() {
            continue;
        }
        match fs::read_to_string(&candidate) {
            Ok(body) => match parse_toml(&body) {
                Ok(adrs) => {
                    return DefaultAcknowledgeConfig {
                        adrs,
                        source: Some(candidate),
                    };
                }
                Err(_) => continue,
            },
            Err(_) => continue,
        }
    }
    DefaultAcknowledgeConfig::default()
}

fn candidate_paths(workdir: &Path) -> Vec<PathBuf> {
    vec![
        workdir.join("product.toml"),
        workdir.join(".product").join("config.toml"),
    ]
}

/// Parse the `[features].default-acknowledged-cross-cutting` list out of
/// a TOML body using a minimal hand-rolled scanner so the module does
/// not pull a serde-toml dependency into the workspace just for one
/// config field.
fn parse_toml(body: &str) -> Result<BTreeSet<String>, String> {
    let mut in_features = false;
    let mut collected = BTreeSet::new();
    let mut continuing: Option<String> = None;
    for raw in body.lines() {
        let trimmed = raw.trim();
        if let Some(buf) = continuing.as_mut() {
            buf.push(' ');
            buf.push_str(trimmed);
            if trimmed.ends_with(']') {
                let full = continuing.take().unwrap_or_default();
                collect_from_inline(&full, &mut collected);
            }
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_features = trimmed == "[features]";
            continue;
        }
        if !in_features {
            continue;
        }
        let Some((key, rest)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key != "default-acknowledged-cross-cutting" {
            continue;
        }
        let value = rest.trim();
        if value.starts_with('[') && value.ends_with(']') {
            collect_from_inline(value, &mut collected);
        } else if value.starts_with('[') {
            continuing = Some(value.to_string());
        } else {
            return Err(format!(
                "expected list literal for default-acknowledged-cross-cutting, got `{value}`"
            ));
        }
    }
    Ok(collected)
}

fn collect_from_inline(text: &str, out: &mut BTreeSet<String>) {
    let inner = text
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    for token in inner.split(',') {
        let token = token.trim().trim_matches('"').trim_matches('\'').trim();
        if !token.is_empty() {
            out.insert(token.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_config_yields_default() {
        let tmp = tempdir();
        let cfg = load_default_acknowledge(&tmp);
        assert!(cfg.adrs.is_empty(), "{cfg:?}");
        assert!(cfg.source.is_none());
    }

    #[test]
    fn product_toml_inline_list_parses() {
        let tmp = tempdir();
        std::fs::write(
            tmp.join("product.toml"),
            "[features]\ndefault-acknowledged-cross-cutting = [\"ADR-001\", \"ADR-013\"]\n",
        )
        .expect("write product.toml");
        let cfg = load_default_acknowledge(&tmp);
        assert!(cfg.acknowledges("ADR-001"));
        assert!(cfg.acknowledges("ADR-013"));
        assert!(!cfg.acknowledges("ADR-999"));
        assert!(cfg.source.is_some());
    }

    #[test]
    fn dot_product_config_toml_also_supported() {
        let tmp = tempdir();
        std::fs::create_dir_all(tmp.join(".product")).expect("mkdir .product");
        std::fs::write(
            tmp.join(".product").join("config.toml"),
            "[features]\n\
             default-acknowledged-cross-cutting = [\n  \"ADR-002\",\n  \"ADR-005\",\n]\n",
        )
        .expect("write config.toml");
        let cfg = load_default_acknowledge(&tmp);
        assert!(cfg.acknowledges("ADR-002"));
        assert!(cfg.acknowledges("ADR-005"));
    }

    #[test]
    fn empty_list_behaves_like_absent() {
        let tmp = tempdir();
        std::fs::write(
            tmp.join("product.toml"),
            "[features]\ndefault-acknowledged-cross-cutting = []\n",
        )
        .expect("write product.toml");
        let cfg = load_default_acknowledge(&tmp);
        assert!(cfg.adrs.is_empty());
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut base = std::env::temp_dir();
        base.push(format!("decision-cli-ft104-cfg-{}-{nanos}-{n}", std::process::id()));
        std::fs::create_dir_all(&base).expect("create tempdir");
        base
    }
}
