//! FT-099 MCP registration helpers — split from `cli/mcp.rs` to keep
//! the parent file under ADR-013 §Rule 1's 400-line hard cap.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use decision_cli::core::handler::Request;
use decision_cli::core::mcp::{RegisterError, ToolDescriptor, ToolHandler, ToolRegistry};
use decision_cli::verify_feature;
use decision_cli::verify_graph_run;

/// Register `dec_verify_graph_run`.
pub fn register_verify_graph_run(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_graph_run::tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_graph_run::parse_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_graph_run::run(&parsed)?;
        Ok(verify_graph_run::response_for(&outcome))
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

/// Register `dec_verify_feature`.
pub fn register_verify_feature(
    registry: &mut ToolRegistry,
    workdir: &Path,
) -> Result<(), RegisterError> {
    let base = verify_feature::tool_descriptor();
    let workdir_owned: PathBuf = workdir.to_path_buf();
    let bound: ToolHandler = Arc::new(move |req: Request| {
        let mut parsed = verify_feature::parse_request(&req)?;
        if parsed.workdir.is_none() {
            parsed.workdir = Some(workdir_owned.clone());
        }
        let outcome = verify_feature::run(&parsed)?;
        Ok(verify_feature::response_for(&outcome))
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
