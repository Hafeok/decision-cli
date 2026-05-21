//! FT-049 MCP registration helpers — split from `cli/mcp.rs` to keep the
//! parent file under ADR-013 §Rule 1's 400-line hard cap.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use decision_cli::core::handler::Request;
use decision_cli::core::mcp::{RegisterError, ToolDescriptor, ToolHandler, ToolRegistry};
use decision_cli::verify_graph_generate;

/// Register `dec_verify_graph_generate`.
pub fn register_verify_graph_generate(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_graph_generate::generate_tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_graph_generate::parse_generate_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_graph_generate::run_generate(&parsed)?;
        Ok(verify_graph_generate::response_for_generate(&outcome))
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

/// Register `dec_verify_graph_accept`.
pub fn register_verify_graph_accept(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_graph_generate::accept_tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_graph_generate::parse_accept_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_graph_generate::run_accept(&parsed)?;
        Ok(verify_graph_generate::response_for_accept(&outcome))
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
