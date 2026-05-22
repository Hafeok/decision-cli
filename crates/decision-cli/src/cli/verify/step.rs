//! `dec verify step add` — clap adapter for the step-add surface (FT-044).
//!
//! Per ADR-029, the CLI subcommand and its MCP twin (`dec_verify_step_add`)
//! both route through `features::verify_step_add::run`. This file is wiring
//! only.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::verify_step_add::{self, StepAddRequest};

use super::exit_code_for;

#[derive(Debug, Subcommand)]
pub enum StepCmd {
    /// Append a typed step to an existing VerificationGraph (FT-044).
    Add(StepAddArgs),
}

#[derive(Debug, clap::Args)]
pub struct StepAddArgs {
    /// Identifier of the target graph (e.g. `VG-001` or `VG-001-foo`).
    pub graph_id: String,
    /// Step kind: shell-command | sparql-assertion | file-assertion |
    /// http-request | wait-for | capture.
    #[arg(long, value_name = "KIND")]
    pub r#type: String,
    /// Per-kind field, repeatable. Format: `key=value`. Allowed keys
    /// depend on `--type`; see `dec verify step add --help` (or the
    /// MCP tool schema) for the per-kind set.
    #[arg(long = "field", value_name = "KEY=VALUE")]
    pub fields: Vec<String>,
}

/// Convert clap args into the structured [`StepAddRequest`]. Exposed so
/// the TC-052 surface-symmetry test can construct the same request the
/// binary does without invoking the binary.
pub fn step_add_request(
    args: &StepAddArgs,
    workdir: &Path,
) -> Result<StepAddRequest, HandlerError> {
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    for raw in &args.fields {
        let (key, value) = parse_field_pair(raw)?;
        if fields.insert(key.clone(), value).is_some() {
            return Err(HandlerError::InvalidArgument {
                field: format!("fields.{key}"),
                detail: format!("--field {key}=... supplied more than once"),
            });
        }
    }
    Ok(StepAddRequest {
        graph_id: args.graph_id.clone(),
        step_type: args.r#type.clone(),
        fields,
        provides_evidence_for: Vec::new(),
        workdir: Some(workdir.to_path_buf()),
    })
}

fn parse_field_pair(raw: &str) -> Result<(String, String), HandlerError> {
    let mut iter = raw.splitn(2, '=');
    let key = iter
        .next()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "field".to_string(),
            detail: format!("--field must be KEY=VALUE; got {raw:?}"),
        })?
        .trim();
    let value = iter.next().ok_or_else(|| HandlerError::InvalidArgument {
        field: "field".to_string(),
        detail: format!("--field must be KEY=VALUE; got {raw:?}"),
    })?;
    if key.is_empty() {
        return Err(HandlerError::InvalidArgument {
            field: "field".to_string(),
            detail: "field key must be non-empty".to_string(),
        });
    }
    Ok((key.to_string(), value.to_string()))
}

pub(super) fn run(workdir: &Path, cmd: StepCmd) -> ExitCode {
    match cmd {
        StepCmd::Add(args) => run_step_add(workdir, args),
    }
}

fn run_step_add(workdir: &Path, args: StepAddArgs) -> ExitCode {
    let req = match step_add_request(&args, workdir) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("dec verify step add: {err}");
            return ExitCode::from(exit_code_for(&err));
        }
    };
    match verify_step_add::run(&req) {
        Ok(outcome) => {
            println!(
                "Appended step {step} at position {pos}",
                step = outcome.step_id,
                pos = outcome.position
            );
            println!("  Graph: {}", outcome.graph_path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("dec verify step add: {err}");
            ExitCode::from(exit_code_for(&err))
        }
    }
}
