//! `dec mcp serve` — clap adapter (FT-034 / ADR-029).
//!
//! Two responsibilities, both wiring per ADR-013 §Rule 3:
//!
//!   1. Parse the subcommand args via clap.
//!   2. Compose the MCP tool registry from feature modules — this is
//!      the binary's job per ADR-016 (sibling features must not import
//!      each other; the binary is the composition root).
//!
//! Every production tool is registered here. New `dec verify *` (or
//! other content-management) features add a single line below pointing
//! at their `feature::tool_descriptor()` plus any workdir binding.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use clap::Subcommand;
use serde_json::json;

use decision_cli::core::handler::{Error as HandlerError, Request, Response};
use decision_cli::core::mcp::{
    RegisterError, ToolDescriptor, ToolHandler, ToolRegistry,
};
use decision_cli::mcp;
use decision_cli::verify_env_new;

#[derive(Debug, Subcommand)]
pub enum McpCmd {
    /// Run the MCP server over stdio.
    Serve,
}

pub fn run(workdir: &Path, cmd: McpCmd) -> ExitCode {
    match cmd {
        McpCmd::Serve => match build_and_serve(workdir) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("dec mcp serve: {err}");
                ExitCode::from(1)
            }
        },
    }
}

fn build_and_serve(workdir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let registry = build_production_registry(workdir)?;
    mcp::serve_with_registry(workdir, registry).map_err(Box::from)
}

/// Assemble every production MCP tool the binary exposes.
///
/// Starts with [`mcp::build_registry`] (gives fixture support when
/// `DEC_MCP_TEST_FIXTURES=1`) and appends each feature module's
/// `tool_descriptor()`, binding the workdir into the handler closure so
/// MCP invocations target the server's launch directory.
fn build_production_registry(workdir: &Path) -> Result<ToolRegistry, RegisterError> {
    let mut registry = mcp::build_registry(workdir)
        .map_err(|e| match e {
            mcp::McpError::Register(r) => r,
            mcp::McpError::Serve(_) => unreachable!("build_registry never returns Serve"),
        })?;
    register_verify_env_new(&mut registry, workdir)?;
    Ok(registry)
}

/// FT-038: register `dec_verify_env_new`. Future verify subcommands
/// add their own register-* call here.
fn register_verify_env_new(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_env_new::tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_env_new::parse_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_env_new::run(&parsed)?;
        Ok(verify_env_new_response(&outcome))
    });
    let mut descriptor = ToolDescriptor::new(
        base.name.clone(),
        base.description.clone(),
        base.input_schema.clone(),
        bound,
    );
    if let Some(schema) = base.output_schema.clone() {
        descriptor = descriptor.with_output_schema(schema);
    }
    registry.register(descriptor)?;
    Ok(())
}

fn verify_env_new_response(outcome: &verify_env_new::EnvNewResponse) -> Response {
    let summary = format!(
        "created env {id} at {path}",
        id = outcome.id,
        path = outcome.path.display()
    );
    Response::with_summary(
        json!({
            "id": outcome.id,
            "path": outcome.path,
        }),
        summary,
    )
}

// `HandlerError` import retained for symmetry — handlers above may
// surface it through the ToolHandler signature.
#[allow(dead_code)]
fn _retain_handler_error(_e: HandlerError) {}
