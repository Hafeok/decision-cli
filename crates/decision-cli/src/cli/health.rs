//! `dec health` — ontology / store / writer liveness gates (FT-012).
//!
//! All three gates run; failures aggregate so the operator sees the
//! full picture. Exit code 0 iff every applicable gate passed.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::health as health_cmd;

pub fn run(workdir: &Path) -> ExitCode {
    let report = health_cmd::check(workdir);
    print!("{}", report.render());
    if report.is_healthy() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
