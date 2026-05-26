//! `dec verify feature` — clap adapter for the feature roll-up verb (FT-099).
//!
//! Per ADR-029, the CLI subcommand and its MCP twin
//! (`dec_verify_feature`) both route through
//! `features::verify_feature::run`. This file is wiring only.

use std::path::Path;
use std::process::ExitCode;

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::verify_feature::{
    self, FeatureVerifyRequest, OutputFormat as FeatureFormat,
};

use super::exit_code_for;

#[derive(Debug, clap::Args)]
pub struct FeatureArgs {
    /// Feature id (e.g. `FT-099`).
    pub feature_id: String,
    /// Filter to one environment (`ENV-NNN[-suffix]`).
    #[arg(long)]
    pub environment: Option<String>,
    /// Output format. Defaults to `text`.
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub format: String,
    /// Skip Feedback emission for failing evidence-bearing steps.
    #[arg(long = "no-feedback")]
    pub no_feedback: bool,
    /// Consider VGRs older than the freshness window.
    #[arg(long = "include-stale")]
    pub include_stale: bool,
    /// Enumerate which graphs would run; do not execute.
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

/// Build a [`FeatureVerifyRequest`] from clap args.
pub fn feature_verify_request(
    args: &FeatureArgs,
    workdir: &Path,
) -> Result<FeatureVerifyRequest, HandlerError> {
    let _ = FeatureFormat::parse(&args.format).ok_or_else(|| HandlerError::InvalidArgument {
        field: "format".to_string(),
        detail: format!(
            "format must be one of {{text, json}}; got {got:?}",
            got = args.format
        ),
    })?;
    Ok(FeatureVerifyRequest {
        feature_id: args.feature_id.clone(),
        environment_id: args.environment.clone(),
        no_feedback: args.no_feedback,
        include_stale: args.include_stale,
        dry_run: args.dry_run,
        workdir: Some(workdir.to_path_buf()),
    })
}

pub fn run_feature(workdir: &Path, args: FeatureArgs) -> ExitCode {
    let format_requested = args.format.clone();
    let req = match feature_verify_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify feature: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_feature::run(&req) {
        Ok(resp) => {
            let format = FeatureFormat::parse(&format_requested).unwrap_or_default();
            match format {
                FeatureFormat::Text => print!("{}", verify_feature::render_text(&resp)),
                FeatureFormat::Json => println!("{}", verify_feature::render_json(&resp)),
            }
            ExitCode::from(resp.exit_code())
        }
        Err(err) => {
            eprintln!("dec verify feature: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}
