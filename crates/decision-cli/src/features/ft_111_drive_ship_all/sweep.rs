//! Multi-feature sweep orchestration (FT-111 §PAT-003).
//!
//! Implements per-item bounded execution with structured outcomes and
//! tally. Used by `dec drive ship --all` to run the ship driver against
//! every feature in the product graph.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::core::drive::{ArtifactRef, Goal, PlanContext};
use crate::drive::{run, DriveError, RunArgs};

/// Per-feature outcome classification.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outcome {
    Done,
    Stuck { reason: String },
    #[serde(rename = "max_iter")]
    HitMaxIter,
    Timeout { after_secs: u64 },
    Error { detail: String },
}

/// Per-feature row in the sweep result.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub feature_id: String,
    pub outcome: Outcome,
    pub iterations: usize,
    pub elapsed_ms: u64,
}

/// Aggregate tally across all features in the sweep.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Tally {
    pub done: usize,
    pub stuck: usize,
    pub max_iter: usize,
    pub timeout: usize,
    pub error: usize,
}

impl Tally {
    /// Increment the appropriate counter based on outcome.
    fn bump(&mut self, outcome: &Outcome) {
        match outcome {
            Outcome::Done => self.done += 1,
            Outcome::Stuck { .. } => self.stuck += 1,
            Outcome::HitMaxIter => self.max_iter += 1,
            Outcome::Timeout { .. } => self.timeout += 1,
            Outcome::Error { .. } => self.error += 1,
        }
    }
}

/// Sweep configuration passed to `run_sweep`.
#[derive(Debug, Clone)]
pub struct SweepInput {
    pub features: Vec<String>,
    pub env_id: Option<String>,
    pub max_iter: usize,
    pub per_item_timeout: Duration,
}

/// Error variants for sweep-level failures (not per-feature failures).
#[derive(Debug, thiserror::Error)]
pub enum SweepError {
    #[error("no features found in product graph")]
    NoFeatures,
    #[error("unknown feature IDs in filter: {0}")]
    UnknownFilterIds(String),
    #[error("failed to initialize plan context: {0}")]
    PlanContext(String),
}

/// Run the multi-feature sweep.
///
/// Iterates each feature in `input.features`, runs the ship driver under
/// a timeout, collects the outcome, and continues past per-item failures.
/// Returns `(rows, tally)` where `tally` is derived from `rows`.
pub async fn run_sweep(
    workdir: PathBuf,
    input: SweepInput,
) -> Result<(Vec<Row>, Tally), SweepError> {
    let mut rows = Vec::with_capacity(input.features.len());

    for ft in &input.features {
        let started = Instant::now();
        let artifact = ArtifactRef {
            short_id: ft.clone(),
            kind: crate::core::drive::ArtifactKind::Feature,
        };

        // Build a fresh context per feature
        let ctx = match PlanContext::open(workdir.clone(), None, input.env_id.clone()) {
            Ok(c) => c,
            Err(e) => {
                // Context error is a sweep-level failure, but we continue
                let outcome = Outcome::Error {
                    detail: format!("plan context: {e}"),
                };
                rows.push(Row {
                    feature_id: ft.clone(),
                    outcome,
                    iterations: 0,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                });
                continue;
            }
        };

        let run_args = RunArgs {
            goal: Goal::Ship,
            artifact,
            max_iter: input.max_iter,
        };

        // Run the drive under timeout
        let outcome = match tokio::time::timeout(
            input.per_item_timeout,
            tokio::task::spawn_blocking({
                let ctx = ctx.clone();
                let args = run_args.clone();
                move || run(&ctx, &args)
            }),
        )
        .await
        {
            Ok(Ok(Ok(drive_outcome))) => {
                // Success
                rows.push(Row {
                    feature_id: ft.clone(),
                    outcome: Outcome::Done,
                    iterations: drive_outcome.iterations,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                });
                continue;
            }
            Ok(Ok(Err(DriveError::Stuck { reason, history }))) => {
                let _iterations = history.len().saturating_sub(1);
                Outcome::Stuck { reason }
            }
            Ok(Ok(Err(DriveError::MaxIterations { history, .. }))) => {
                let _iterations = history.len().saturating_sub(1);
                Outcome::HitMaxIter
            }
            Ok(Ok(Err(e))) => Outcome::Error {
                detail: format!("{e:#}"),
            },
            Ok(Err(_)) => Outcome::Error {
                detail: "drive task panicked".to_string(),
            },
            Err(_elapsed) => Outcome::Timeout {
                after_secs: input.per_item_timeout.as_secs(),
            },
        };

        let iterations = match &outcome {
            Outcome::Done => unreachable!("handled above"),
            Outcome::Stuck { .. } | Outcome::HitMaxIter => {
                // Approximation: we don't have the history in scope here
                0
            }
            Outcome::Timeout { .. } | Outcome::Error { .. } => 0,
        };

        rows.push(Row {
            feature_id: ft.clone(),
            outcome,
            iterations,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }

    // Derive tally from rows
    let mut tally = Tally::default();
    for row in &rows {
        tally.bump(&row.outcome);
    }

    Ok((rows, tally))
}

/// Resolve the list of features from the product graph.
///
/// Reads `.product/features/` and returns feature IDs sorted by numeric
/// suffix ascending (FT-2, FT-3, FT-10, not FT-10, FT-2, FT-3).
pub fn resolve_features(product_root: &Path) -> Result<Vec<String>, SweepError> {
    let features_dir = product_root.join("features");
    let entries = std::fs::read_dir(&features_dir).map_err(|e| {
        SweepError::PlanContext(format!("failed to read {}: {e}", features_dir.display()))
    })?;

    let mut features: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| {
            SweepError::PlanContext(format!("failed to read directory entry: {e}"))
        })?;
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        // Extract FT-NNN from filename
        if let Some(ft_id) = filename_str.strip_prefix("FT-").and_then(|s| {
            s.split('-')
                .next()
                .and_then(|num| num.parse::<u32>().ok())
                .map(|n| format!("FT-{n}"))
        }) {
            features.push(ft_id);
        }
    }

    if features.is_empty() {
        return Err(SweepError::NoFeatures);
    }

    // Sort by numeric suffix
    features.sort_by_key(|ft| {
        ft.strip_prefix("FT-")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    Ok(features)
}

/// Filter the resolved feature list against an optional allowlist.
///
/// Returns the intersection of `resolved` and `filter`, preserving the
/// order from `resolved`. Errors if any filter ID is not in `resolved`.
pub fn apply_filter(
    resolved: Vec<String>,
    filter: Option<Vec<String>>,
) -> Result<Vec<String>, SweepError> {
    let Some(filter) = filter else {
        return Ok(resolved);
    };

    if filter.is_empty() {
        return Err(SweepError::UnknownFilterIds(
            "empty filter list".to_string(),
        ));
    }

    let resolved_set: std::collections::HashSet<_> = resolved.iter().collect();
    let mut unknown: Vec<String> = Vec::new();

    for ft in &filter {
        if !resolved_set.contains(ft) {
            unknown.push(ft.clone());
        }
    }

    if !unknown.is_empty() {
        return Err(SweepError::UnknownFilterIds(unknown.join(", ")));
    }

    // Intersection preserving resolved order
    Ok(resolved
        .into_iter()
        .filter(|ft| filter.contains(ft))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_serialization_stable() {
        let done = Outcome::Done;
        let json = serde_json::to_string(&done).unwrap();
        assert_eq!(json, r#"{"type":"done"}"#);

        let stuck = Outcome::Stuck {
            reason: "test".to_string(),
        };
        let json = serde_json::to_string(&stuck).unwrap();
        assert!(json.contains(r#""type":"stuck""#));
        assert!(json.contains(r#""reason":"test""#));
    }

    #[test]
    fn tally_bump_increments_correct_counter() {
        let mut tally = Tally::default();
        tally.bump(&Outcome::Done);
        tally.bump(&Outcome::Done);
        tally.bump(&Outcome::Stuck {
            reason: "test".to_string(),
        });
        tally.bump(&Outcome::HitMaxIter);
        tally.bump(&Outcome::Timeout { after_secs: 600 });
        tally.bump(&Outcome::Error {
            detail: "test".to_string(),
        });

        assert_eq!(tally.done, 2);
        assert_eq!(tally.stuck, 1);
        assert_eq!(tally.max_iter, 1);
        assert_eq!(tally.timeout, 1);
        assert_eq!(tally.error, 1);
    }

    #[test]
    fn apply_filter_preserves_resolved_order() {
        let resolved = vec![
            "FT-2".to_string(),
            "FT-3".to_string(),
            "FT-10".to_string(),
            "FT-100".to_string(),
        ];
        let filter = vec!["FT-10".to_string(), "FT-2".to_string()];
        let result = apply_filter(resolved, Some(filter)).unwrap();
        assert_eq!(result, vec!["FT-2", "FT-10"]);
    }

    #[test]
    fn apply_filter_errors_on_unknown_ids() {
        let resolved = vec!["FT-2".to_string(), "FT-3".to_string()];
        let filter = vec!["FT-2".to_string(), "FT-999".to_string()];
        let result = apply_filter(resolved, Some(filter));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("FT-999"));
    }

    #[test]
    fn apply_filter_errors_on_empty_filter() {
        let resolved = vec!["FT-2".to_string()];
        let filter = vec![];
        let result = apply_filter(resolved, Some(filter));
        assert!(result.is_err());
    }
}
