//! `dec init` validation pipeline (FT-008 / ADR-006).
//!
//! FT-006 owns the ontology + shapes, but the test criteria the
//! pipeline gates on (TC-001, TC-003) exercise the whole init flow.
//! This module implements the minimal pipeline needed to make those
//! TCs pass:
//!
//! ```text
//!   parse → SHACL-validate → resolve → cross-validate → persist → provenance
//! ```
//!
//! Each step is a separate function so FT-008 / FT-009 can swap in a
//! richer implementation without touching the CLI plumbing.
//!
//! Failure modes are structured via [`InitError`] so the CLI can name
//! exactly which field / IRI / goal was wrong (ADR-006).

pub mod config;
mod generate;
mod parse;
mod persist;
mod pipeline;
pub mod safety;
mod shacl;
mod sparql;
mod vocab;

use std::path::{Path, PathBuf};

use chrono::Utc;
use thiserror::Error;

use crate::core::ontology::{OntologyError, OntologyHandle};

use parse::read_definition_bytes;
use persist::{finalise_orchestration_dir, json_escape, sha256_hex};
use pipeline::{build_orchestration_store, stage_and_validate};

pub use safety::GitignoreOutcome;

/// IRI of the bootstrap session record (`dec:session/init-001`).
pub const BOOTSTRAP_SESSION_IRI: &str = "https://decision-cli.dev/ns/session/init-001";

/// Named graph used to hold the persisted orchestration state on init.
pub const ORCHESTRATION_GRAPH_IRI: &str = "https://decision-cli.dev/ns/orchestration";

/// Distinguishes the `dec init` input shapes (ADR-006 §3.2, extended by FT-114).
#[derive(Debug, Clone)]
pub enum DefinitionSource {
    /// `dec init --template <name>` — bundled stream template.
    Template(String),
    /// `dec init --from <path>` — local Turtle file.
    File(PathBuf),
    /// `dec init` (no args) — auto-discover from `.product/` (FT-114).
    AutoDiscover,
}

impl DefinitionSource {
    fn label(&self) -> String {
        match self {
            Self::Template(n) => format!("template:{n}"),
            Self::File(p) => p.display().to_string(),
            Self::AutoDiscover => "auto-discovered".to_string(),
        }
    }

    fn form(&self) -> &'static str {
        match self {
            Self::Template(_) => "template",
            Self::File(_) => "file",
            Self::AutoDiscover => "auto-discover",
        }
    }
}

/// Output of a successful [`run`].
#[derive(Debug, Clone)]
pub struct InitOutcome {
    /// IRI of the persisted ValueStream artifact.
    pub stream_iri: String,
    /// IRI of the persisted ValueAction artifact.
    pub value_action_iri: String,
    /// Bootstrap session IRI (always `dec:session/init-001`).
    pub session_iri: String,
    /// SHA-256 (hex) of the input definition bytes (template *or* file).
    pub definition_hash: String,
    /// Source label as recorded on the session.
    pub definition_source: String,
    /// Ontology version recorded on the session.
    pub ontology_version: String,
    /// Authorized goals copied off the validated stream.
    pub authorized_goals: Vec<String>,
    /// Path to the persisted store directory (`.dec/store/`).
    pub store_dir: PathBuf,
    /// Path to the serialized N-Quads dump inside `store_dir`.
    pub store_dump_path: PathBuf,
}

/// Structured init failures (ADR-006).
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum InitError {
    #[error("already initialised: {0} exists; refusing to overwrite")]
    AlreadyInitialised(PathBuf),

    #[error("unknown bundled template '{name}'; available: {available}")]
    UnknownTemplate { name: String, available: String },

    #[error("failed to read definition file {path}: {source}")]
    ReadFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse Turtle from {source_label}: {detail}")]
    ParseFailed {
        source_label: String,
        detail: String,
    },

    #[error("SHACL validation failed for {source_label}:\n{report}")]
    ShaclViolation {
        source_label: String,
        report: String,
    },

    #[error(
        "ValueAction <{iri}> is not bundled with slice 1; bundled URIs: {available}. \
         Network resolution is deferred (decision-cli-slice-1-bounds.md §6.2)."
    )]
    UnknownValueAction { iri: String, available: String },

    #[error(
        "authorized goal {goal:?} is not in the compatible-goals set for {value_action} \
         (compatible: {compatible}). Per ADR-005: this stream pursues {value_action}; \
         try a stream whose value-action accepts {goal:?}."
    )]
    UnauthorizedGoal {
        goal: String,
        value_action: String,
        compatible: String,
    },

    #[error("ontology unavailable: {0}")]
    Ontology(#[from] OntologyError),

    #[error("failed to persist orchestration store: {0}")]
    PersistFailed(String),

    #[error("internal store error: {0}")]
    Internal(String),
}

/// Run the full init pipeline against the supplied source, persisting
/// the orchestration store under `<workdir>/.dec/store/`.
///
/// FT-114: Idempotent — re-running init refreshes the stream and re-seeds
/// the store without wiping existing session/dispatch state.
pub fn run(workdir: &Path, source: DefinitionSource) -> Result<InitOutcome, InitError> {
    run_with_opts(workdir, source, false, false)
}

/// Run init with FT-114 options for .env and gitignore safety checks.
pub fn run_with_opts(
    workdir: &Path,
    source: DefinitionSource,
    auto_confirm: bool,
    skip_env_check: bool,
) -> Result<InitOutcome, InitError> {
    let dec_dir = workdir.join(".dec");

    // FT-114: If .dec/ exists, we're in idempotent mode.
    // The store will be merged, not replaced.
    let is_reinit = dec_dir.exists();

    let ontology = OntologyHandle::load()?;
    let (definition_bytes, source_label, base_iri) = read_definition_bytes(workdir, &source)?;
    let definition_hash = sha256_hex(&definition_bytes);
    let staged = stage_and_validate(&definition_bytes, &source_label, base_iri.as_deref())?;

    persist_init_artifacts(
        workdir,
        &dec_dir,
        &staged,
        &source,
        &source_label,
        &definition_hash,
        &definition_bytes,
        ontology.version(),
        is_reinit,
    )?;

    // FT-114: Safety checks after store is persisted
    if !skip_env_check {
        safety::bootstrap_env_example(workdir)?;
        safety::ensure_env_gitignored(workdir, auto_confirm)?;
    }

    Ok(build_init_outcome(
        &dec_dir,
        staged,
        definition_hash,
        source_label,
        ontology.version().to_string(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn persist_init_artifacts(
    workdir: &Path,
    dec_dir: &Path,
    staged: &pipeline::StagedDefinition,
    source: &DefinitionSource,
    source_label: &str,
    definition_hash: &str,
    definition_bytes: &[u8],
    ontology_version: &str,
    is_reinit: bool,
) -> Result<(), InitError> {
    let now = Utc::now().to_rfc3339();
    let orchestration = build_orchestration_store(
        staged,
        source_label,
        definition_hash,
        ontology_version,
        source.form(),
        &now,
    )?;
    let metadata_json = build_init_metadata_json(
        source_label,
        definition_hash,
        ontology_version,
        source.form(),
        &staged.stream_iri,
        &staged.terminal_iri,
    );
    finalise_orchestration_dir(
        workdir,
        dec_dir,
        &orchestration,
        definition_bytes,
        &metadata_json,
        source,
        is_reinit,
    )
}

fn build_init_metadata_json(
    source_label: &str,
    definition_hash: &str,
    ontology_version: &str,
    form: &str,
    stream_iri: &str,
    value_action_iri: &str,
) -> String {
    format!(
        r#"{{"source":"{source}","hash":"{hash}","ontology_version":"{ver}","form":"{form}","stream":"{stream}","value_action":"{va}"}}"#,
        source = json_escape(source_label),
        hash = definition_hash,
        ver = ontology_version,
        form = form,
        stream = stream_iri,
        va = value_action_iri,
    )
}

fn build_init_outcome(
    dec_dir: &Path,
    staged: pipeline::StagedDefinition,
    definition_hash: String,
    source_label: String,
    ontology_version: String,
) -> InitOutcome {
    let store_dir = dec_dir.join("store");
    let dump_path = store_dir.join("orchestration.nq");
    InitOutcome {
        stream_iri: staged.stream_iri,
        value_action_iri: staged.terminal_iri,
        session_iri: BOOTSTRAP_SESSION_IRI.to_string(),
        definition_hash,
        definition_source: source_label,
        ontology_version,
        authorized_goals: staged.authorized,
        store_dir,
        store_dump_path: dump_path,
    }
}
