//! `dec mcp serve` — clap adapter (FT-034 / ADR-029).
//!
//! Pure wiring: parses the subcommand arguments and delegates to
//! [`decision_cli::mcp::serve`]. Per ADR-013 §Rule 3 this file
//! contains no business logic.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;
use decision_cli::mcp;

#[derive(Debug, Subcommand)]
pub enum McpCmd {
    /// Run the MCP server over stdio.
    Serve,
}

pub fn run(workdir: &Path, cmd: McpCmd) -> ExitCode {
    match cmd {
        McpCmd::Serve => match mcp::serve(workdir) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("dec mcp serve: {err}");
                ExitCode::from(1)
            }
        },
    }
}
