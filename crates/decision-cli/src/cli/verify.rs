//! `dec verify {env,...}` — CLI adapter for the verification surface.
//!
//! Slice 2.5 ships the first verification subcommand, `dec verify env new`
//! (FT-038). The clap tree mirrors the MCP tool-name path: each leaf
//! subcommand maps 1:1 to a `dec_verify_<noun>_<verb>` tool. Per ADR-029
//! the CLI and MCP surfaces route through the same handler; this module
//! only translates clap args into the structured [`EnvNewRequest`].

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::verify_env_new::{self, EnvNewRequest};

/// Names of every MCP tool the `dec verify` clap tree pairs with. The
/// TC-052 surface-symmetry harness asserts this list matches the MCP
/// registry. The constant is `pub` so the parity TC's structural check
/// (grep over this file) can confirm one entry per leaf subcommand.
#[allow(dead_code)]
pub const PAIRED_TOOL_NAMES: &[&str] = &["dec_verify_env_new"];

#[derive(Debug, Subcommand)]
pub enum VerifyCmd {
    /// Manage `dec:VerificationEnvironment` artifacts.
    #[command(subcommand)]
    Env(EnvCmd),
}

#[derive(Debug, Subcommand)]
pub enum EnvCmd {
    /// Create a new VerificationEnvironment (FT-038).
    New(EnvNewArgs),
}

#[derive(Debug, clap::Args)]
pub struct EnvNewArgs {
    /// Caller-supplied id (e.g. ENV-007). Omitted → mints the next free ENV-NNN.
    #[arg(long)]
    pub id: Option<String>,
    /// Environment type tag (e.g. `ephemeral-tempdir`, `remote-http`).
    #[arg(long = "type", value_name = "ENV-TYPE")]
    pub env_type: String,
    /// Safety class: `isolated`, `shared-non-destructive`, or `production-readonly`.
    #[arg(long = "safety-class")]
    pub safety_class: String,
    /// Comma-separated operation tokens permitted in the env (e.g. `shell,filesystem`).
    #[arg(long = "allowed-ops")]
    pub allowed_ops: String,
    /// Optional setup shell snippet.
    #[arg(long)]
    pub setup: Option<String>,
    /// Optional teardown shell snippet.
    #[arg(long)]
    pub teardown: Option<String>,
    /// Required iff `--type` is `remote-*`; forbidden for local types.
    #[arg(long)]
    pub endpoint: Option<String>,
}

/// Convert clap args into the structured `EnvNewRequest`. Exposed so
/// the TC-052 unit test can build the same request the binary does.
#[must_use]
pub fn env_new_request(args: &EnvNewArgs, workdir: &Path) -> EnvNewRequest {
    let allowed_ops: Vec<String> = args
        .allowed_ops
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    EnvNewRequest {
        id: args.id.clone(),
        env_type: args.env_type.clone(),
        safety_class: args.safety_class.clone(),
        allowed_ops,
        setup: args.setup.clone(),
        teardown: args.teardown.clone(),
        endpoint: args.endpoint.clone(),
        workdir: Some(workdir.to_path_buf()),
    }
}

pub fn run(workdir: &Path, cmd: VerifyCmd) -> ExitCode {
    match cmd {
        VerifyCmd::Env(env_cmd) => match env_cmd {
            EnvCmd::New(args) => run_env_new(workdir, args),
        },
    }
}

fn run_env_new(workdir: &Path, args: EnvNewArgs) -> ExitCode {
    let req = env_new_request(&args, workdir);
    match verify_env_new::run(&req) {
        Ok(outcome) => {
            println!("Created VerificationEnvironment {id}", id = outcome.id);
            println!("  Path: {}", outcome.path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify env new: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

/// Map handler errors to CLI exit codes per FT-038 §Error handling.
fn exit_code_for(err: &HandlerError) -> u8 {
    match err {
        HandlerError::InvalidArgument { .. } => 2,
        _ => 1,
    }
}
