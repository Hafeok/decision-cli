//! `dec verify env {new, list, show}` — clap adapter for the env surface.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::verify_env_list::{self, EnvListRequest, OutputFormat as ListFormat};
use decision_cli::verify_env_new::{self, EnvNewRequest};
use decision_cli::verify_env_show::{self, EnvShowRequest, OutputFormat as ShowFormat};

use super::exit_code_for;

#[derive(Debug, Subcommand)]
pub enum EnvCmd {
    /// Create a new VerificationEnvironment (FT-038).
    New(EnvNewArgs),
    /// List VerificationEnvironment artifacts (FT-039).
    List(EnvListArgs),
    /// Show a single VerificationEnvironment in detail (FT-040).
    Show(EnvShowArgs),
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
pub fn env_list_request(
    args: &EnvListArgs,
    workdir: &Path,
) -> Result<EnvListRequest, HandlerError> {
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
pub fn env_show_request(
    args: &EnvShowArgs,
    workdir: &Path,
) -> Result<EnvShowRequest, HandlerError> {
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

/// Convert clap args into the structured `EnvNewRequest`.
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

pub(super) fn run(workdir: &Path, cmd: EnvCmd) -> ExitCode {
    match cmd {
        EnvCmd::New(args) => run_env_new(workdir, args),
        EnvCmd::List(args) => run_env_list(workdir, args),
        EnvCmd::Show(args) => run_env_show(workdir, args),
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
