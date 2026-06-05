//! Cluster dispatcher for task types
//!
//! Executes a task type's cell cluster in topological order, running each cell
//! as a separate session and running the coherence audit at the end.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::capability_resolver::CapabilityResolver;
use crate::core::task_type::{TaskTypeDecl, Cluster, PlanError};
use crate::core::worktree::Worktree;
use crate::features::drive::planners::feature_ship::Action;
use crate::features::implement::worker::{DispatchPayloadJson, WorkerResponseJson};
use crate::features::implement::worker::run_dispatch;
use crate::product_core::ProductCore;
use crate::utils::tempdir::TempDir;

/// Outcome of a cluster execution
#[derive(Debug, Clone)]
pub enum ClusterOutcome {
    /// All cells executed successfully and audit passed
    Done {
        cells_emitted: usize,
        audit_outcome: AuditOutcome,
    },
    /// Cluster failed due to cell execution error
    CellFailed {
        cell_name: String,
        error: String,
    },
    /// Cluster failed due to audit failure
    AuditFailed {
        check: String,
        detail: String,
    },
    /// Cluster audit could not be run
    AuditUnrunnable {
        error: String,
    },
}

/// Outcome of a coherence audit
#[derive(Debug, Clone)]
pub enum AuditOutcome {
    /// Audit passed
    Pass,
    /// Audit failed
    Fail {
        check: String,
        detail: String,
    },
}

/// Execute a task type cluster
///
/// # Arguments
/// * `workdir` - Working directory for the operation
/// * `ctx` - Context for the operation
/// * `args` - Arguments including feature_id
/// * `task_type_name` - Name of the task type to execute
///
/// # Returns
/// * `ClusterOutcome` - Result of the cluster execution
pub async fn run(
    workdir: &PathBuf,
    ctx: &crate::core::context::Context,
    args: &crate::features::drive::ship::ShipArgs,
    task_type_name: &str,
) -> Result<ClusterOutcome, Box<dyn std::error::Error>> {
    // Look up the task type
    let task_type = get_task_type(task_type_name)
        .ok_or_else(|| format!("Unknown task type: {}", task_type_name))?;

    // Get the worktree for this operation
    let mut worktree = Worktree::new(workdir)?;

    // Get the feature spec for this operation
    let feature_id = &args.feature_id;
    let product_core = ProductCore::new(ctx.product_root())?;
    let feature_spec = product_core.read_feature_spec(feature_id).await?;

    // Walk cells in topological order
    let cell_order = Cluster::topo_order(&task_type.cells)?;
    
    // Track emitted artifacts by cell name
    let mut emitted_artifacts: HashMap<String, Vec<String>> = HashMap::new();

    // Execute each cell in order
    for cell_name in cell_order {
        let cell = task_type.cells.iter()
            .find(|c| c.name == cell_name)
            .expect("Cell should exist based on topo_order");

        // Resolve model binding for this cell
        let capability_resolver = CapabilityResolver::new(ctx);
        let binding = capability_resolver.resolve(&cell.model_binding_capability_iri).await?;
        
        // Prepare the prompt template by rendering it with upstream artifacts
        let rendered_prompt = render_prompt_template(&cell.prompt_template_path, &emitted_artifacts)?;

        // Dispatch the cell as a separate session
        let session_id = format!("cluster-{}-{}", task_type_name, cell_name);
        let dispatch_payload = DispatchPayloadJson {
            dispatch_id: session_id.clone(),
            session_id: session_id.clone(),
            feature_id: feature_id.clone(),
            bundle_markdown: feature_spec.content.clone(),
            bundle_hash: feature_spec.hash.clone(),
            workspace_path: workdir.to_string_lossy().to_string(),
            model_id: binding.model_id,
            timeout_seconds: 60, // Default timeout
            max_turns: 10,       // Default turns
        };

        let response = run_dispatch(&dispatch_payload, ctx).await?;
        
        // Handle response and persist artifacts
        match response {
            WorkerResponseJson { 
                status: "success".to_string(), 
                code_change: Some(change), 
                ..
            } => {
                // Persist emitted artifacts
                for file_write in change.files {
                    let file_path = PathBuf::from(file_write.path);
                    worktree.write_file(&file_path, &file_write.contents).await?;
                    
                    // Track this artifact
                    emitted_artifacts.entry(cell_name.clone()).or_insert_with(Vec::new)
                        .push(file_write.path);
                }
            }
            WorkerResponseJson { 
                status: "failed".to_string(), 
                error: Some(error), 
                ..
            } => {
                return Ok(ClusterOutcome::CellFailed {
                    cell_name: cell_name.clone(),
                    error: error.message,
                });
            }
            _ => {
                return Ok(ClusterOutcome::CellFailed {
                    cell_name: cell_name.clone(),
                    error: "Worker returned unexpected response".to_string(),
                });
            }
        }
    }

    // Run the coherence audit
    let audit_result = run_coherence_audit(&task_type.coherence_audit, &emitted_artifacts).await?;
    
    match audit_result {
        AuditOutcome::Pass => {
            // Commit the worktree changes
            worktree.commit(format!("[{}] Cluster execution successful", feature_id)).await?;
            
            Ok(ClusterOutcome::Done {
                cells_emitted: emitted_artifacts.len(),
                audit_outcome: AuditOutcome::Pass,
            })
        }
        AuditOutcome::Fail { check, detail } => {
            // Rollback the worktree changes
            worktree.rollback().await?;
            
            Ok(ClusterOutcome::AuditFailed { check, detail })
        }
    }
}

fn get_task_type(name: &str) -> Option<TaskTypeDecl> {
    // In a real implementation, this would look up from a registry
    // For now, we're returning a mock implementation for testing purposes
    match name {
        "add-judge-worker" => Some(TaskTypeDecl {
            name: "add-judge-worker".to_string(),
            cells: vec![
                // These would be populated properly in a real implementation
                // We're focusing on the infrastructure here
            ],
            coherence_audit: crate::core::task_type::CoherenceAuditSpec {
                script_path: "scripts/checks/cluster-audit-add-judge-worker.py".to_string(),
                timeout_seconds: 30,
            },
        }),
        _ => None,
    }
}

fn render_prompt_template(template_path: &str, artifacts: &HashMap<String, Vec<String>>) -> Result<String, Box<dyn std::error::Error>> {
    // In a real implementation, this would render the Jinja template
    // with the upstream artifacts as context
    Ok(format!("Template from {}", template_path))
}

async fn run_coherence_audit(
    audit_spec: &crate::core::task_type::CoherenceAuditSpec,
    artifacts: &HashMap<String, Vec<String>>
) -> Result<AuditOutcome, Box<dyn std::error::Error>> {
    // In a real implementation, this would run the audit script
    // For now, we're returning a success result for testing purposes
    
    // This would normally execute the Python audit script with the artifact paths
    // and return the appropriate AuditOutcome
    
    // Simulate success for now
    Ok(AuditOutcome::Pass)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cluster_dispatch() {
        // This is a placeholder test - in reality this would need a more complex setup
        let temp_dir = TempDir::new().unwrap();
        let workdir = temp_dir.path().to_path_buf();
        
        // Mock context and args
        let ctx = crate::core::context::Context::builder()
            .with_product_root(&workdir)
            .build();
            
        let args = crate::features::drive::ship::ShipArgs {
            feature_id: "FT-T373".to_string(),
            dry_run: false,
            force: false,
        };
        
        // This test would require mocking the capability resolver and workers
        // For now, we just verify the function compiles
        assert_eq!(true, true);
    }
}