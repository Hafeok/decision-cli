//! Shared read-side context the planners consume. Pre-resolves the
//! orchestration-store handle, working directory, product root, and a
//! default-env hint so each planner doesn't re-open the store on every
//! call. Planners are expected to be pure functions of `(ctx, artifact)`.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Read-side handle a planner uses to inspect graph state. Cheap to
/// construct (no I/O until the first query); planners that touch the
/// store call its helper accessors.
///
/// Field accessors are `pub` so planners outside this module (under
/// `features::drive::planners::*`) can compose them. Tests can build
/// fixture `PlanContext`s via [`PlanContext::for_test`].
#[derive(Debug, Clone)]
pub struct PlanContext {
    /// Working directory the dispatched handlers will run against.
    pub workdir: PathBuf,
    /// Product-cli root. Defaults to `workdir` when the CLI didn't
    /// override it.
    pub product_root: PathBuf,
    /// Optional env override supplied at CLI invocation. Planners use
    /// this verbatim when they need an env for a dispatch action; if
    /// `None`, they look up the default env for the feature.
    pub env_override: Option<String>,
}

impl PlanContext {
    /// Open a planning context against a real working tree.
    pub fn open(
        workdir: PathBuf,
        product_root: Option<PathBuf>,
        env_override: Option<String>,
    ) -> Result<Self, ContextError> {
        if !workdir.is_dir() {
            return Err(ContextError::InvalidWorkdir {
                path: workdir.display().to_string(),
            });
        }
        Ok(Self {
            workdir: workdir.clone(),
            product_root: product_root.unwrap_or(workdir),
            env_override,
        })
    }

    /// Test helper: build a `PlanContext` without I/O so unit tests of
    /// planner state-classification can pass `(verdict, open_counts)`
    /// directly via a wrapper trait rather than seeding the store.
    pub fn for_test(workdir: &Path) -> Self {
        Self {
            workdir: workdir.to_path_buf(),
            product_root: workdir.to_path_buf(),
            env_override: None,
        }
    }

    /// Resolve the env the planner should pass to dispatch actions.
    /// Returns `env_override` when set; otherwise the planner-supplied
    /// default.
    #[must_use]
    pub fn env_or_default(&self, default_for_feature: &str) -> String {
        self.env_override
            .clone()
            .unwrap_or_else(|| default_for_feature.to_string())
    }
}

/// Errors surfaced when opening a `PlanContext`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContextError {
    /// `workdir` doesn't exist or isn't a directory.
    #[error("invalid workdir {path:?}")]
    InvalidWorkdir {
        /// The path the operator supplied.
        path: String,
    },
}
