//! `dec` — decision-cli binary entry point (dispatch-only per ADR-013).
//! Slice 1 surface plus FT-033 feedback CLI. See ADR-011, ADR-016.

mod cli;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let parsed = cli::args::Cli::parse();
    let workdir = parsed
        .workdir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    cli::args::dispatch(&workdir, parsed.command)
}
