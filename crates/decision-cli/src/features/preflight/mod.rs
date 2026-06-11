//! `dec preflight FT-XXX` — graph-sourced feature-coverage report (FT-052).
//!
//! The internal product-cli graph projection (`.product/graph/index.ttl`)
//! is the **source of truth**. The markdown frontmatter under
//! `.product/features/`, `.product/adrs/`, and `.product/tests/` is the
//! *human input*; the projection is the *machine record* the dispatch
//! gate (ADR-031, FT-047) and this command both read.
//!
//! Reading the projection — not the markdown — is the consistency
//! claim that lets `dec` trust the graph for chain-integrity checks
//! without re-parsing markdown on every dispatch. TC-087 asserts the
//! contract: a frontmatter mutation that has not yet been projected
//! into `index.ttl` does **not** influence `dec preflight`.
//!
//! The report carries three sections (matching the structure of
//! `product preflight` on the same key set):
//!
//! - `cross_cutting_gaps`: cross-cutting ADRs the feature has not
//!   linked or acknowledged via the projection.
//! - `domain_gaps`: domain ADRs missing for the feature's domain set.
//! - `dep_availability`: each `pm:dependsOn` feature with its status.

mod query;
mod report;

pub use report::{CrossCuttingRow, DependencyStatus, PreflightReport};

use std::path::{Path, PathBuf};

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::GraphName;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use self::query::iris::{PM, PM_FEATURE};
use self::query::{
    query_cross_cutting, query_dep_availability, query_domain_gaps, split_cross_cutting,
};

/// Errors `dec preflight` may surface to the operator.
#[derive(Debug, Error)]
pub enum PreflightError {
    /// `--workdir` does not contain `.product/graph/index.ttl` and so
    /// the projection cannot be read. Operator hint suggests running
    /// `product graph rebuild`.
    #[error(
        "no graph projection at {path} — run `product graph rebuild` to produce it \
         (FT-052 reads the projection, not the markdown)"
    )]
    ProjectionMissing {
        /// Path the reader looked for.
        path: PathBuf,
    },
    /// The projection file exists but could not be read off disk.
    #[error("read projection {path}: {detail}")]
    ReadFailed {
        /// Path the reader attempted.
        path: PathBuf,
        /// IO error detail.
        detail: String,
    },
    /// The projection file exists but is not valid Turtle.
    #[error("parse projection {path}: {detail}")]
    ParseFailed {
        /// Path the parser attempted.
        path: PathBuf,
        /// Parser error detail.
        detail: String,
    },
    /// An internal Oxigraph error (in-memory store / query). These
    /// should not occur in practice; they are surfaced rather than
    /// panicked.
    #[error("store error: {detail}")]
    StoreError {
        /// Underlying error detail.
        detail: String,
    },
    /// The supplied feature id does not appear in the projection.
    #[error(
        "feature {feature_id} not found in graph projection at {path} \
         (run `product graph rebuild` if the feature was just authored)"
    )]
    FeatureNotProjected {
        /// Feature id the caller asked about.
        feature_id: String,
        /// Path the reader looked at.
        path: PathBuf,
    },
}

/// Run the preflight report against the projection under `workdir`.
///
/// The function reads `.product/graph/index.ttl` from `workdir`,
/// parses it into a fresh in-memory store, and executes three SPARQL
/// queries over it. The markdown files under `.product/` are
/// **not** opened. TC-087 leans on this property.
pub fn run(workdir: &Path, feature_id: &str) -> Result<PreflightReport, PreflightError> {
    let projection_path = projection_path_for(workdir);
    run_with_projection(&projection_path, feature_id)
}

/// Like [`run`] but takes an explicit projection path. Used by
/// integration tests that stage a synthetic projection in a temp dir.
pub fn run_with_projection(
    projection_path: &Path,
    feature_id: &str,
) -> Result<PreflightReport, PreflightError> {
    if !projection_path.exists() {
        return Err(PreflightError::ProjectionMissing {
            path: projection_path.to_path_buf(),
        });
    }
    let bytes = std::fs::read(projection_path).map_err(|e| PreflightError::ReadFailed {
        path: projection_path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let store = load_projection(&bytes, projection_path)?;
    assert_feature_in_projection(&store, feature_id, projection_path)?;
    let cross_cutting = query_cross_cutting(&store, feature_id)?;
    let (linked, gaps) = split_cross_cutting(cross_cutting);
    let domain_gaps = query_domain_gaps(&store, feature_id)?;
    let dep_availability = query_dep_availability(&store, feature_id)?;
    Ok(PreflightReport {
        feature_id: feature_id.to_string(),
        cross_cutting_gaps: gaps,
        cross_cutting_linked: linked,
        domain_gaps,
        dep_availability,
        projection_source: projection_path.to_path_buf(),
    })
}

/// Resolve `<workdir>/.product/graph/index.ttl`. Public so tests in
/// the same crate can stage the path without re-deriving it.
#[must_use]
pub fn projection_path_for(workdir: &Path) -> PathBuf {
    workdir.join(".product").join("graph").join("index.ttl")
}

fn load_projection(bytes: &[u8], path: &Path) -> Result<Store, PreflightError> {
    let store = Store::new().map_err(|e| PreflightError::StoreError {
        detail: e.to_string(),
    })?;
    let parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::DefaultGraph);
    store
        .load_from_reader(parser, bytes)
        .map_err(|e| PreflightError::ParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    Ok(store)
}

fn assert_feature_in_projection(
    store: &Store,
    feature_id: &str,
    path: &Path,
) -> Result<(), PreflightError> {
    let q = format!("ASK {{ <{PM_FEATURE}{feature_id}> a <{PM}Feature> }}");
    let res = store
        .query(q.as_str())
        .map_err(|e| PreflightError::StoreError {
            detail: format!("ASK feature exists: {e}"),
        })?;
    match res {
        QueryResults::Boolean(true) => Ok(()),
        QueryResults::Boolean(false) => Err(PreflightError::FeatureNotProjected {
            feature_id: feature_id.to_string(),
            path: path.to_path_buf(),
        }),
        _ => Err(PreflightError::StoreError {
            detail: "ASK returned non-boolean".into(),
        }),
    }
}
