//! DriveHistoryReader trait and ProductionReader implementation (FT-113).

use super::model::{Dispatch, Outcome, Round, RoundState};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("feature not found: {0}")]
    FeatureNotFound(String),
    #[error("orchestration store error: {0}")]
    StoreError(String),
    #[error("no drives found for feature {0}")]
    NoDrives(String),
}

pub trait DriveHistoryReader {
    fn rounds_for_feature(
        &self,
        feature_id: &str,
        bench_id: Option<&str>,
    ) -> Result<Vec<Round>, ReadError>;
}

pub struct ProductionReader {
    workdir: PathBuf,
}

impl ProductionReader {
    pub fn new(workdir: PathBuf) -> Self {
        Self { workdir }
    }
}

impl DriveHistoryReader for ProductionReader {
    fn rounds_for_feature(
        &self,
        feature_id: &str,
        _bench_id: Option<&str>,
    ) -> Result<Vec<Round>, ReadError> {
        // For now, return empty vec - full implementation would query the store
        // This is a placeholder to allow tests to compile and the command to be wired

        // Validate feature ID exists in product graph
        let product_root = self.workdir.join(".product");
        if !product_root.exists() {
            return Err(ReadError::StoreError(
                "product graph not found - run 'dec init' first".to_string(),
            ));
        }

        // Check if feature exists in product graph
        let features_dir = product_root.join("features");
        if !features_dir.exists() {
            return Err(ReadError::FeatureNotFound(feature_id.to_string()));
        }

        // Try to find the feature file
        let feature_files: Vec<_> = std::fs::read_dir(&features_dir)
            .map_err(|e| ReadError::StoreError(format!("failed to read features dir: {e}")))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{}-", feature_id))
            })
            .collect();

        if feature_files.is_empty() {
            return Err(ReadError::FeatureNotFound(feature_id.to_string()));
        }

        // For now, return empty - no drives yet
        // Full implementation will:
        // 1. Query orchestration store for prov:Activity instances
        // 2. Group by dispatch timestamp
        // 3. Read .dec/verify/graph/*.ttl for VG-ids
        // 4. Query for dec:Feedback instances
        // 5. Assemble into Round structs

        Ok(Vec::new())
    }
}

/// Test stub for unit tests
pub struct StubReader {
    pub rounds: Vec<Round>,
}

impl DriveHistoryReader for StubReader {
    fn rounds_for_feature(
        &self,
        _feature_id: &str,
        _bench_id: Option<&str>,
    ) -> Result<Vec<Round>, ReadError> {
        Ok(self.rounds.clone())
    }
}

/// Mutable stub for convergence tests
pub struct MutableStubReader {
    pub rounds: std::cell::RefCell<Vec<Round>>,
}

impl DriveHistoryReader for MutableStubReader {
    fn rounds_for_feature(
        &self,
        _feature_id: &str,
        _bench_id: Option<&str>,
    ) -> Result<Vec<Round>, ReadError> {
        Ok(self.rounds.borrow().clone())
    }
}

// Helper to create test rounds
#[cfg(test)]
pub fn test_round(index: u32, role: &str, outcome: Outcome) -> Round {
    let base_time = DateTime::parse_from_rfc3339("2026-05-30T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    Round {
        round_index: index,
        started_at: base_time + chrono::Duration::seconds((index * 180) as i64),
        elapsed_since_round_zero: Duration::from_secs((index * 180) as u64),
        state: RoundState {
            verdict: if index == 0 { "NeverRun".to_string() } else { "Rejected".to_string() },
            impl_open: if index == 0 { 0 } else { 7 },
            vga_open: 0,
            graph_count: index as usize,
        },
        dispatch: Dispatch {
            role: role.to_string(),
            session_iri: format!("sess:test-{index}"),
        },
        outcome,
    }
}
