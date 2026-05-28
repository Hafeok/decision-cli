//! FT-109 — audit-trail reporters for `dec loop show`/`dec loop list`.
//!
//! Operator-facing views over the verify → re-fix loop that FT-107 +
//! FT-108 wired up. The data lives in the orchestration store today —
//! every `dec:Feedback` carries `source_session`, `source_artifact`,
//! `addressing_artifact`, and `lifecycle_state` — but reconstructing
//! what happened across the loop requires walking those links by hand.
//! This module rolls them up.
//!
//! Two surfaces:
//!   * [`run_show`] — one feature, chronological chain of every defect
//!     feedback tied to its TCs.
//!   * [`run_list`] — overview across all features, rolled-up open vs
//!     closed counts.

pub mod render;
pub mod resolver;
pub mod show;

use std::path::Path;
use std::process::ExitCode;

use crate::core::handler::Error as HandlerError;

pub use render::OutputFormat;
pub use show::{LoopEntry, LoopShowRequest, LoopShowResponse};

/// CLI dispatch for `dec loop show <FT-NNN>`.
pub fn run_show(workdir: &Path, args: &ShowArgs) -> ExitCode {
    let req = LoopShowRequest {
        feature_id: args.feature_id.clone(),
        workdir: workdir.to_path_buf(),
        product_root: args.product_root.clone(),
        format: args.format,
    };
    match show::run(&req) {
        Ok(resp) => {
            print!("{}", render::show_response(&resp, args.format));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec loop show: {err}");
            ExitCode::from(1)
        }
    }
}

/// CLI dispatch for `dec loop list`.
pub fn run_list(workdir: &Path, args: &ListArgs) -> ExitCode {
    let req = list::LoopListRequest {
        workdir: workdir.to_path_buf(),
        product_root: args.product_root.clone(),
        state: args.state,
        format: args.format,
    };
    match list::run(&req) {
        Ok(resp) => {
            print!("{}", render::list_response(&resp, args.format));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec loop list: {err}");
            ExitCode::from(1)
        }
    }
}

pub mod list;

#[derive(Debug, Clone)]
pub struct ShowArgs {
    pub feature_id: String,
    pub product_root: Option<std::path::PathBuf>,
    pub format: OutputFormat,
}

#[derive(Debug, Clone)]
pub struct ListArgs {
    pub state: list::StateFilter,
    pub product_root: Option<std::path::PathBuf>,
    pub format: OutputFormat,
}

/// Shared error funnel — surfaces the underlying read error to the CLI.
fn handler_internal(detail: String) -> HandlerError {
    HandlerError::Internal { detail }
}
