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
use decision_cli::verify_env_list::{self, EnvListRequest, OutputFormat as ListFormat};
use decision_cli::verify_env_new::{self, EnvNewRequest};
use decision_cli::verify_env_show::{self, EnvShowRequest, OutputFormat as ShowFormat};
use decision_cli::verify_graph_list::{
    self, GraphListRequest, OutputFormat as GraphListFormat,
};
use decision_cli::verify_graph_new::{self, GraphNewRequest};

/// Names of every MCP tool the `dec verify` clap tree pairs with. The
/// TC-052 surface-symmetry harness asserts this list matches the MCP
/// registry. The constant is `pub` so the parity TC's structural check
/// (grep over this file) can confirm one entry per leaf subcommand.
#[allow(dead_code)]
pub const PAIRED_TOOL_NAMES: &[&str] = &[
    "dec_verify_env_list",
    "dec_verify_env_new",
    "dec_verify_env_show",
    "dec_verify_graph_list",
    "dec_verify_graph_new",
];

#[derive(Debug, Subcommand)]
pub enum VerifyCmd {
    /// Manage `dec:VerificationEnvironment` artifacts.
    #[command(subcommand)]
    Env(EnvCmd),
    /// Manage `dec:VerificationGraph` artifacts.
    #[command(subcommand)]
    Graph(GraphCmd),
}

#[derive(Debug, Subcommand)]
pub enum EnvCmd {
    /// Create a new VerificationEnvironment (FT-038).
    New(EnvNewArgs),
    /// List VerificationEnvironment artifacts (FT-039).
    List(EnvListArgs),
    /// Show a single VerificationEnvironment in detail (FT-040).
    Show(EnvShowArgs),
}

#[derive(Debug, Subcommand)]
pub enum GraphCmd {
    /// Create a new VerificationGraph (FT-041).
    New(GraphNewArgs),
    /// List VerificationGraph artifacts (FT-042).
    List(GraphListArgs),
}

#[derive(Debug, clap::Args)]
pub struct GraphListArgs {
    /// Optional filter — `FT-NNN` or `TC-NNN` reference.
    #[arg(long)]
    pub verifies: Option<String>,
    /// Optional environment filter (e.g. `ENV-001-ephemeral-cli`).
    #[arg(long)]
    pub environment: Option<String>,
    /// Output format. Defaults to `table`.
    #[arg(long, value_name = "FORMAT", default_value = "table")]
    pub format: String,
}

#[derive(Debug, clap::Args)]
pub struct GraphNewArgs {
    /// Caller-supplied id (e.g. VG-007). Omitted → mints the next free VG-NNN.
    #[arg(long)]
    pub id: Option<String>,
    /// Feature or TC the graph proves about (e.g. `FT-001`, `TC-013`).
    #[arg(long)]
    pub verifies: String,
    /// VerificationEnvironment the graph executes against (e.g. `ENV-001-ephemeral-cli`).
    #[arg(long)]
    pub environment: String,
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

#[derive(Debug, clap::Args)]
pub struct EnvListArgs {
    /// Optional safety-class filter (`isolated`, `shared-non-destructive`,
    /// or `production-readonly`).
    #[arg(long = "safety-class")]
    pub safety_class: Option<String>,
    /// Optional env-type filter (e.g. `ephemeral-tempdir`, `remote-http`).
    #[arg(long = "type", value_name = "ENV-TYPE")]
    pub env_type: Option<String>,
    /// Output format. Defaults to `table`.
    #[arg(long, value_name = "FORMAT", default_value = "table")]
    pub format: String,
}

#[derive(Debug, clap::Args)]
pub struct EnvShowArgs {
    /// Identifier of the env to show (e.g. `ENV-001-ephemeral-cli`).
    pub id: String,
    /// Output format. Defaults to `text`.
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub format: String,
}

/// Convert env-list clap args into the structured [`EnvListRequest`].
///
/// Returns `Err` for malformed `--format` values so the CLI surface can
/// short-circuit before invoking the handler.
pub fn env_list_request(args: &EnvListArgs, workdir: &Path) -> Result<EnvListRequest, HandlerError> {
    let format = ListFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
        field: "format".to_string(),
        detail: format!(
            "format must be one of {{table, json}}; got {got:?}",
            got = args.format
        ),
    })?;
    Ok(EnvListRequest {
        safety_class: args.safety_class.clone(),
        env_type: args.env_type.clone(),
        format: Some(format),
        workdir: Some(workdir.to_path_buf()),
    })
}

/// Convert env-show clap args into the structured [`EnvShowRequest`].
///
/// Returns `Err` for malformed `--format` values so the CLI surface can
/// short-circuit before invoking the handler (FT-040 §Error handling).
pub fn env_show_request(args: &EnvShowArgs, workdir: &Path) -> Result<EnvShowRequest, HandlerError> {
    let format = ShowFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
        field: "format".to_string(),
        detail: format!(
            "format must be one of {{text, json}}; got {got:?}",
            got = args.format
        ),
    })?;
    Ok(EnvShowRequest {
        id: args.id.clone(),
        format: Some(format),
        workdir: Some(workdir.to_path_buf()),
    })
}

/// Convert graph-list clap args into the structured [`GraphListRequest`].
///
/// Returns `Err` for malformed `--format` values so the CLI surface can
/// short-circuit before invoking the handler.
pub fn graph_list_request(
    args: &GraphListArgs,
    workdir: &Path,
) -> Result<GraphListRequest, HandlerError> {
    let format =
        GraphListFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
            field: "format".to_string(),
            detail: format!(
                "format must be one of {{table, json}}; got {got:?}",
                got = args.format
            ),
        })?;
    Ok(GraphListRequest {
        verifies: args.verifies.clone(),
        environment: args.environment.clone(),
        format: Some(format),
        workdir: Some(workdir.to_path_buf()),
    })
}

/// Convert clap args into the structured `GraphNewRequest`. Exposed so
/// the TC-052 unit test can build the same request the binary does.
#[must_use]
pub fn graph_new_request(args: &GraphNewArgs, workdir: &Path) -> GraphNewRequest {
    GraphNewRequest {
        id: args.id.clone(),
        verifies: args.verifies.clone(),
        environment: args.environment.clone(),
        workdir: Some(workdir.to_path_buf()),
    }
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
            EnvCmd::List(args) => run_env_list(workdir, args),
            EnvCmd::Show(args) => run_env_show(workdir, args),
        },
        VerifyCmd::Graph(graph_cmd) => match graph_cmd {
            GraphCmd::New(args) => run_graph_new(workdir, args),
            GraphCmd::List(args) => run_graph_list(workdir, args),
        },
    }
}

fn run_graph_list(workdir: &Path, args: GraphListArgs) -> ExitCode {
    let req = match graph_list_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify graph list: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_graph_list::run(&req) {
        Ok(outcome) => {
            let format = req.format.unwrap_or_default();
            match format {
                GraphListFormat::Table => {
                    print!("{}", verify_graph_list::render_table(&outcome))
                }
                GraphListFormat::Json => {
                    println!("{}", verify_graph_list::render_json(&outcome))
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify graph list: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

fn run_graph_new(workdir: &Path, args: GraphNewArgs) -> ExitCode {
    let req = graph_new_request(&args, workdir);
    match verify_graph_new::run(&req) {
        Ok(outcome) => {
            println!("Created VerificationGraph {id}", id = outcome.id);
            println!("  Path: {}", outcome.path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify graph new: {err}");
            ExitCode::from(exit_code_for(&err))
        }
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

fn run_env_list(workdir: &Path, args: EnvListArgs) -> ExitCode {
    let req = match env_list_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify env list: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_env_list::run(&req) {
        Ok(outcome) => {
            let format = req.format.unwrap_or_default();
            match format {
                ListFormat::Table => print!("{}", verify_env_list::render_table(&outcome)),
                ListFormat::Json => println!("{}", verify_env_list::render_json(&outcome)),
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify env list: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}

fn run_env_show(workdir: &Path, args: EnvShowArgs) -> ExitCode {
    let req = match env_show_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify env show: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_env_show::run(&req) {
        Ok(outcome) => {
            let format = req.format.unwrap_or_default();
            match format {
                ShowFormat::Text => print!("{}", verify_env_show::render_text(&outcome)),
                ShowFormat::Json => println!("{}", verify_env_show::render_json(&outcome)),
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify env show: {err}");
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
