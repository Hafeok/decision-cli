//! `dec feedback {list,…}` — feedback inspection (FT-029 / FT-033).

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;
use decision_cli::feedback;

#[derive(Debug, Subcommand)]
pub enum FeedbackCmd {
    /// List open feedback grouped by class and target role (FT-029 / TC-039).
    List,
}

pub fn run(workdir: &Path, cmd: FeedbackCmd) -> ExitCode {
    match cmd {
        FeedbackCmd::List => run_list(workdir),
    }
}

fn run_list(workdir: &Path) -> ExitCode {
    match feedback::list(workdir) {
        Ok(rows) => {
            print!("{}", feedback::format_list(&rows));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec feedback list: {err:#}");
            ExitCode::from(1)
        }
    }
}
