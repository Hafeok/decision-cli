//! Bootstrap seeds for `dec init` per FT-035 §Behaviour step 6.
//!
//! The `ephemeral-cli` env is the default substrate consumed by the
//! verification-graph executor in slice 3; seeding it here makes the
//! first verification graph authorable without a separate setup step.

use super::types::{SafetyClass, VerificationEnvironment};

/// Canonical id of the bootstrap-seeded ephemeral env.
pub const EPHEMERAL_CLI_ENV_ID: &str = "ENV-001-ephemeral-cli";

/// File name (under `.dec/verify/env/`) the seed env persists at.
pub const EPHEMERAL_CLI_ENV_FILENAME: &str = "ENV-001-ephemeral-cli.ttl";

/// Construct the canonical `ephemeral-cli` env. Stable across runs so
/// re-running `dec init` produces byte-identical Turtle (TC-055).
#[must_use]
pub fn ephemeral_cli_env() -> VerificationEnvironment {
    VerificationEnvironment {
        id: EPHEMERAL_CLI_ENV_ID.to_string(),
        env_type: "ephemeral-tempdir".to_string(),
        setup: Some("mkdir -p \"$TMPDIR\" && cd \"$TMPDIR\"".to_string()),
        teardown: Some("rm -rf \"$TMPDIR\"".to_string()),
        allowed_ops: vec![
            "shell".to_string(),
            "filesystem".to_string(),
            "sparql-local".to_string(),
        ],
        safety_class: SafetyClass::Isolated,
        endpoint: None,
    }
}
