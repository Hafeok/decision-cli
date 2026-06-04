//! `dec drive` orchestrator + concrete planners (FT-110).
//!
//! Driver loop lives in [`run`]; planners live under [`planners`].
//! Public surface kept narrow so the CLI module composes against a
//! stable boundary.

pub mod cluster_dispatch;
pub mod execute;
pub mod inspect;
pub(crate) mod inspect_dor;
pub mod outcome;
pub mod planners;
pub mod registry;
pub mod run;

pub use execute::{Executor, ProductionExecutor};
pub use outcome::{DriveError, DriveOutcome, HistoryEntry};
pub use registry::planner_for;
pub use run::{run, run_with_executor, run_with_planner_and_executor, RunArgs, DEFAULT_MAX_ITER};
