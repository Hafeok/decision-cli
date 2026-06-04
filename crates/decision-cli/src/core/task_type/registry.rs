//! Static TaskType registry (FT-139 / ADR-080).
//!
//! Populated at startup with the first TaskType — `add-judge-worker`
//! — that ADR-080 designates as the load-bearing prototype. Future
//! slices (FT-140..FT-144 already authored) extend this list.

use std::path::PathBuf;
use std::sync::OnceLock;

use super::types::{CellDecl, CoherenceAuditSpec, TaskTypeDecl};

/// Returns the static TaskType registry, lazily built on first
/// access.
fn registry() -> &'static Vec<TaskTypeDecl> {
    static REGISTRY: OnceLock<Vec<TaskTypeDecl>> = OnceLock::new();
    REGISTRY.get_or_init(|| vec![add_judge_worker()])
}

/// Look up a TaskType by name.
#[must_use]
pub fn lookup(name: &str) -> Option<&'static TaskTypeDecl> {
    registry().iter().find(|tt| tt.name == name)
}

/// Names of every registered TaskType. Stable order matches the
/// registry's insertion order.
#[must_use]
pub fn registered_names() -> Vec<&'static str> {
    registry().iter().map(|tt| tt.name.as_str()).collect()
}

/// The `add-judge-worker` TaskType — five cells (capability_binding →
/// pydantic_io_models → system_prompt → agent_loop → unit_tests),
/// declaration per FT-139 §Outputs.
fn add_judge_worker() -> TaskTypeDecl {
    TaskTypeDecl {
        name: "add-judge-worker".to_string(),
        cells: vec![
            CellDecl {
                name: "capability_binding".to_string(),
                artifact_type: "n-quads".to_string(),
                prompt_template_path: PathBuf::from(
                    "crates/decision-cli/src/core/task_type/templates/add_judge_worker/\
                     capability_binding.tmpl",
                ),
                model_binding_capability_id: String::new(), // mechanical / no LLM
                derived_from: Vec::new(),
            },
            CellDecl {
                name: "pydantic_io_models".to_string(),
                artifact_type: "python-module".to_string(),
                prompt_template_path: PathBuf::from(
                    "crates/decision-cli/src/core/task_type/templates/add_judge_worker/\
                     pydantic_io_models.tmpl",
                ),
                model_binding_capability_id: "code-writer".to_string(),
                derived_from: Vec::new(),
            },
            CellDecl {
                name: "system_prompt".to_string(),
                artifact_type: "markdown".to_string(),
                prompt_template_path: PathBuf::from(
                    "crates/decision-cli/src/core/task_type/templates/add_judge_worker/\
                     system_prompt.tmpl",
                ),
                model_binding_capability_id: "code-writer".to_string(),
                derived_from: vec!["pydantic_io_models".to_string()],
            },
            CellDecl {
                name: "agent_loop".to_string(),
                artifact_type: "python-module".to_string(),
                prompt_template_path: PathBuf::from(
                    "crates/decision-cli/src/core/task_type/templates/add_judge_worker/\
                     agent_loop.tmpl",
                ),
                model_binding_capability_id: "code-writer".to_string(),
                derived_from: vec![
                    "pydantic_io_models".to_string(),
                    "system_prompt".to_string(),
                    "capability_binding".to_string(),
                ],
            },
            CellDecl {
                name: "unit_tests".to_string(),
                artifact_type: "python-module".to_string(),
                prompt_template_path: PathBuf::from(
                    "crates/decision-cli/src/core/task_type/templates/add_judge_worker/\
                     unit_tests.tmpl",
                ),
                model_binding_capability_id: "code-writer".to_string(),
                derived_from: vec![
                    "pydantic_io_models".to_string(),
                    "system_prompt".to_string(),
                ],
            },
        ],
        coherence_audit: CoherenceAuditSpec {
            script_path: PathBuf::from("scripts/checks/cluster-audit-add-judge-worker.py"),
            timeout_seconds: 60,
        },
    }
}
