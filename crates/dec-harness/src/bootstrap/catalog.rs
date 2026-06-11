//! End-to-end catalog bootstrap (FT-058 / ADR-036).
//!
//! Coordinates the YAML→typed parse, the divergence check against the
//! existing graph, the SHACL-validated atomic commit, and the optional
//! bundle / session migration. Lower-level helpers live in the sibling
//! `parse`, `plan`, `diff`, and `migrate` modules.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use dec_graph::store::{load_store_from_dump, orchestration_dump_path, persist_store};

use super::diff::FieldDiff;
use super::migrate::{migrate_bundle_stakes, migrate_session_token_breakdown, MigrateError};
use super::parse::{load_bindings_doc, load_capabilities_doc, parse_bindings, parse_capabilities};
use super::plan::{commit_all, plan_binding_writes, plan_capability_writes};

/// Structured failures from [`bootstrap_catalog`].
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("failed to read YAML file {path}: {detail}")]
    ReadFailed { path: PathBuf, detail: String },

    #[error("failed to parse YAML in {path}: {detail}")]
    YamlParseFailed { path: PathBuf, detail: String },

    #[error("invalid value in {path}: {detail}")]
    InvalidValue { path: PathBuf, detail: String },

    #[error(
        "unresolved capability reference: binding `{binding}` references \
         capability `{missing_capability}` which is not present in \
         config/capabilities.yaml"
    )]
    UnresolvedReference {
        binding: String,
        missing_capability: String,
    },

    #[error(
        "divergence between YAML and graph for {artifact}:\n{diff}\n\
         the graph is authoritative; update the YAML to match, or bump \
         the YAML `version` field and re-run to create a new versioned \
         artifact alongside"
    )]
    Divergence {
        artifact: String,
        diff: String,
        diffs: Vec<FieldDiff>,
    },

    #[error("SHACL violation for {artifact}:\n{report}")]
    ShaclViolated { artifact: String, report: String },

    #[error("failed to read orchestration store at {path}: {detail}")]
    StoreOpenFailed { path: PathBuf, detail: String },

    #[error("failed to persist orchestration store: {detail}")]
    PersistFailed { detail: String },

    #[error("migration failed: {0}")]
    Migrate(#[from] MigrateError),

    #[error("internal store error: {0}")]
    Internal(String),
}

/// What happened during a bootstrap run.
#[derive(Debug, Clone, Default)]
pub struct BootstrapReport {
    /// Capability artifacts newly written this run.
    pub capabilities_written: usize,
    /// Capability artifacts already present, content matched, skipped.
    pub capabilities_skipped: usize,
    /// Role binding artifacts newly written.
    pub bindings_written: usize,
    /// Role binding artifacts already present, content matched, skipped.
    pub bindings_skipped: usize,
    /// Bundles backfilled with `dec:stakes = "routine"`.
    pub bundles_migrated: usize,
    /// Sessions backfilled with the three token-breakdown fields.
    pub sessions_migrated: usize,
    /// SHA-256 (hex) of the canonicalised capabilities YAML.
    pub capabilities_source_hash: String,
    /// SHA-256 (hex) of the canonicalised role-bindings YAML.
    pub bindings_source_hash: String,
}

impl BootstrapReport {
    /// True iff the run made no graph mutations.
    #[must_use]
    pub fn is_unchanged(&self) -> bool {
        self.capabilities_written == 0
            && self.bindings_written == 0
            && self.bundles_migrated == 0
            && self.sessions_migrated == 0
    }
}

/// Run the full bootstrap against an orchestration store rooted at `workdir`.
///
/// `workdir/.dec/store/orchestration.nq` must exist; the helper does not
/// initialise a fresh store. Pass `migrate = true` to also backfill
/// `dec:stakes` and the session token-breakdown fields.
pub fn bootstrap_catalog(
    workdir: &Path,
    capabilities_yaml: &Path,
    bindings_yaml: &Path,
    migrate: bool,
    dry_run: bool,
) -> Result<BootstrapReport, BootstrapError> {
    let (cap_doc, cap_hash) = load_capabilities_doc(capabilities_yaml)?;
    let (bind_doc, bind_hash) = load_bindings_doc(bindings_yaml)?;

    let capabilities = parse_capabilities(&cap_doc, capabilities_yaml, &cap_hash)?;
    let bindings = parse_bindings(&bind_doc, bindings_yaml, &capabilities, &bind_hash)?;

    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump).map_err(|e| BootstrapError::StoreOpenFailed {
        path: dump.clone(),
        detail: e.to_string(),
    })?;

    let mut report = BootstrapReport {
        capabilities_source_hash: cap_hash,
        bindings_source_hash: bind_hash,
        ..Default::default()
    };

    let cap_quads = plan_capability_writes(&store, &capabilities, &mut report)?;
    let (bind_quads, deactivate_quads) = plan_binding_writes(&store, &bindings, &mut report)?;

    commit_all(&store, &cap_quads, &bind_quads, &deactivate_quads)?;

    let store_arc = Arc::new(store);
    if migrate {
        report.bundles_migrated = migrate_bundle_stakes(&store_arc)?;
        report.sessions_migrated = migrate_session_token_breakdown(&store_arc)?;
    }
    if !dry_run {
        persist_store(&store_arc, &dump).map_err(|e| BootstrapError::PersistFailed {
            detail: e.to_string(),
        })?;
    }

    Ok(report)
}
