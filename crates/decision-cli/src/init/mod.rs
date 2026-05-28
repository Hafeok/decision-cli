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

mod parse;
mod persist;
mod pipeline;
mod shacl;
mod sparql;
mod vocab;

use std::path::{Path, PathBuf};

use chrono::Utc;
use thiserror::Error;

use crate::ontology::{OntologyError, OntologyHandle};

use parse::read_definition_bytes;
use persist::{finalise_orchestration_dir, json_escape, sha256_hex};
use pipeline::{build_orchestration_store, stage_and_validate};

/// IRI of the bootstrap session record (`dec:session/init-001`).
pub const BOOTSTRAP_SESSION_IRI: &str = "https://decision-cli.dev/ns/session/init-001";

/// Named graph used to hold the persisted orchestration state on init.
pub const ORCHESTRATION_GRAPH_IRI: &str = "https://decision-cli.dev/ns/orchestration";

/// Distinguishes the two `dec init` input shapes (ADR-006 §3.2).
#[derive(Debug, Clone)]
pub enum DefinitionSource {
    /// `dec init --template <name>` — bundled stream template.
    Template(String),
    /// `dec init --from <path>` — local Turtle file.
    File(PathBuf),
}

impl DefinitionSource {
    fn label(&self) -> String {
        match self {
            Self::Template(n) => format!("template:{n}"),
            Self::File(p) => p.display().to_string(),
        }
    }

    fn form(&self) -> &'static str {
        match self {
            Self::Template(_) => "template",
            Self::File(_) => "file",
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
    ShaclViolation { source_label: String, report: String },

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
/// Errors leave `workdir/.dec/` **absent** — no partial state on
/// failure (TC-003 / TC-004 / TC-005).
pub fn run(workdir: &Path, source: DefinitionSource) -> Result<InitOutcome, InitError> {
    let dec_dir = workdir.join(".dec");
    if dec_dir.exists() {
        return Err(InitError::AlreadyInitialised(dec_dir));
    }

    let ontology = OntologyHandle::load()?;
    let (definition_bytes, source_label, base_iri) = read_definition_bytes(&source)?;
    let definition_hash = sha256_hex(&definition_bytes);

    let staged = stage_and_validate(&definition_bytes, &source_label, base_iri.as_deref())?;

    let now = Utc::now().to_rfc3339();
    let orchestration = build_orchestration_store(
        &staged,
        &source_label,
        &definition_hash,
        ontology.version(),
        source.form(),
        &now,
    )?;

    let metadata_json = format!(
        r#"{{"source":"{source}","hash":"{hash}","ontology_version":"{ver}","form":"{form}","stream":"{stream}","value_action":"{va}"}}"#,
        source = json_escape(&source_label),
        hash = definition_hash,
        ver = ontology.version(),
        form = source.form(),
        stream = staged.stream_iri,
        va = staged.terminal_iri,
    );
    finalise_orchestration_dir(
        workdir,
        &dec_dir,
        &orchestration,
        &definition_bytes,
        &metadata_json,
    )?;

    let store_dir = dec_dir.join("store");
    let dump_path = store_dir.join("orchestration.nq");
    Ok(InitOutcome {
        stream_iri: staged.stream_iri,
        value_action_iri: staged.terminal_iri,
        session_iri: BOOTSTRAP_SESSION_IRI.to_string(),
        definition_hash,
        definition_source: source_label,
        ontology_version: ontology.version().to_string(),
        authorized_goals: staged.authorized,
        store_dir,
        store_dump_path: dump_path,
    })
}
