///! config.toml generation (ADR-068).
use std::fs;
use std::path::Path;

use super::InitError;

/// Generate .dec/config.toml with ADR-068 defaults.
/// Idempotent: if it already exists, leave it unchanged.
pub fn bootstrap_config_toml(dec_dir: &Path) -> Result<bool, InitError> {
    let config_path = dec_dir.join("config.toml");

    if config_path.exists() {
        return Ok(false); // Already exists, don't overwrite
    }

    let template = r#"# .dec/config.toml — Decision-CLI operator preferences (ADR-068).
#
# Precedence chain (highest wins):
#   --flag  >  DEC_* env var  >  this file  >  built-in default
#
# Deleting this file is safe — the CLI falls back to built-in defaults.

[driver]
max_iter = 6                          # planner iteration cap
default_bench = "BNCH-002"            # used when --bench omitted

[sweep]
per_feature_timeout_secs = 600        # `dec drive ship --all` per-item bound
default_format = "text"               # text | tsv | json
max_concurrent_implementers = 1       # FT-115 parallel cap (slice-1 = 1)
auto_retire_failing_graphs = false    # opt-in pre-pass; flag stays --retire-failing-graphs

[show]
watch_interval_secs = 2               # `dec drive show --watch` poll cadence

[init]
default_provider = "scaleway"         # auto-bootstrap provider
default_secret_env = "SCW_SECRET_KEY" # env var name dec init names in .env.example

[paths]
worktree_root = ".dec/worktrees"      # FT-115
store_path = ".dec/store/orchestration.nq"
"#;

    fs::write(&config_path, template)
        .map_err(|e| InitError::PersistFailed(format!("Failed to write config.toml: {}", e)))?;

    Ok(true) // Created
}
