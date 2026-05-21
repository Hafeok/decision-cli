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
use decision_cli::verify_env_list;
use decision_cli::verify_env_new;
use decision_cli::verify_env_show;
use decision_cli::verify_graph_list;
use decision_cli::verify_graph_new;

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
    register_verify_env_list(&mut registry, workdir)?;
    register_verify_env_show(&mut registry, workdir)?;
    register_verify_graph_new(&mut registry, workdir)?;
    register_verify_graph_list(&mut registry, workdir)?;
    Ok(registry)
}

/// FT-042: register `dec_verify_graph_list`. Same workdir-binding pattern
/// the other verify tools use.
fn register_verify_graph_list(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_graph_list::tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_graph_list::parse_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_graph_list::run(&parsed)?;
        Ok(verify_graph_list_response(&outcome))
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

fn verify_graph_list_response(outcome: &verify_graph_list::GraphListResponse) -> Response {
    let summary = format!(
        "listed {n} verification graph(s)",
        n = outcome.graphs.len()
    );
    Response::with_summary(
        json!({
            "graphs": outcome.graphs,
        }),
        summary,
    )
}

/// FT-041: register `dec_verify_graph_new`. Same workdir-binding pattern
/// the env-* tools use.
fn register_verify_graph_new(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_graph_new::tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_graph_new::parse_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_graph_new::run(&parsed)?;
        Ok(verify_graph_new_response(&outcome))
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

fn verify_graph_new_response(outcome: &verify_graph_new::GraphNewResponse) -> Response {
    let summary = format!(
        "created graph {id} at {path}",
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

/// FT-039: register `dec_verify_env_list`. Same workdir-binding pattern
/// as FT-038's `dec_verify_env_new`.
fn register_verify_env_list(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_env_list::tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_env_list::parse_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_env_list::run(&parsed)?;
        Ok(verify_env_list_response(&outcome))
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

fn verify_env_list_response(outcome: &verify_env_list::EnvListResponse) -> Response {
    let summary = format!("listed {n} environment(s)", n = outcome.envs.len());
    Response::with_summary(
        json!({
            "envs": outcome.envs,
        }),
        summary,
    )
}

/// FT-040: register `dec_verify_env_show`. Workdir binding mirrors the
/// list and new patterns above.
fn register_verify_env_show(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_env_show::tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_env_show::parse_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_env_show::run(&parsed)?;
        Ok(verify_env_show_response(&outcome))
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

fn verify_env_show_response(outcome: &verify_env_show::EnvShowResponse) -> Response {
    let summary = format!(
        "showed env {id} from {path}",
        id = outcome.env.id,
        path = outcome.path.display()
    );
    Response::with_summary(
        json!({
            "env": outcome.env,
            "path": outcome.path,
        }),
        summary,
    )
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
