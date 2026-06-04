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
    REGISTRY.get_or_init(|| {
        vec![
            add_judge_worker(),
            add_author_worker(),
            add_artifact_type(),
            add_cli_subcommand(),
            extend_planner_classifier(),
            extend_role_catalog_seed(),
        ]
    })
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

// ---------------------------------------------------------------------
// FT-140 — add-author-worker. Cells (6, with derived_from):
// capability_binding, pydantic_io_models, system_prompt,
// fixtures_example_inputs, agent_loop, unit_tests. Discriminator: the
// Output type carries `body_markdown` not `verdict` — catches
// misclassification with add-judge-worker.
// ---------------------------------------------------------------------

fn add_author_worker() -> TaskTypeDecl {
    let here = "crates/decision-cli/src/core/task_type/templates/add_author_worker";
    TaskTypeDecl {
        name: "add-author-worker".to_string(),
        cells: vec![
            cell("capability_binding", "n-quads", here, "", &[]),
            cell("pydantic_io_models", "python-module", here, "code-writer", &[]),
            cell("system_prompt", "markdown", here, "code-writer", &["pydantic_io_models"]),
            cell("fixtures_example_inputs", "json-fixtures", here, "", &["pydantic_io_models"]),
            cell(
                "agent_loop",
                "python-module",
                here,
                "code-writer",
                &["pydantic_io_models", "system_prompt", "capability_binding"],
            ),
            cell(
                "unit_tests",
                "python-module",
                here,
                "code-writer",
                &["pydantic_io_models", "fixtures_example_inputs", "system_prompt"],
            ),
        ],
        coherence_audit: CoherenceAuditSpec {
            script_path: PathBuf::from("scripts/checks/cluster-audit-add-author-worker.py"),
            timeout_seconds: 60,
        },
    }
}

// ---------------------------------------------------------------------
// FT-141 — add-artifact-type. Cells (6): rust_struct, shacl_shape,
// iri_module_consts, parser, emitter, round_trip_tests. Discriminator:
// no .py files — touches Rust + Turtle only.
// ---------------------------------------------------------------------

fn add_artifact_type() -> TaskTypeDecl {
    let here = "crates/decision-cli/src/core/task_type/templates/add_artifact_type";
    TaskTypeDecl {
        name: "add-artifact-type".to_string(),
        cells: vec![
            cell("rust_struct", "rust-source", here, "code-writer", &[]),
            cell("shacl_shape", "turtle", here, "code-writer", &["rust_struct"]),
            cell("iri_module_consts", "rust-source", here, "", &["rust_struct"]),
            cell(
                "parser",
                "rust-source",
                here,
                "code-writer",
                &["rust_struct", "iri_module_consts"],
            ),
            cell(
                "emitter",
                "rust-source",
                here,
                "code-writer",
                &["rust_struct", "iri_module_consts"],
            ),
            cell(
                "round_trip_tests",
                "rust-source",
                here,
                "code-writer",
                &["rust_struct", "shacl_shape", "parser", "emitter"],
            ),
        ],
        coherence_audit: CoherenceAuditSpec {
            script_path: PathBuf::from("scripts/checks/cluster-audit-add-artifact-type.py"),
            timeout_seconds: 60,
        },
    }
}

// ---------------------------------------------------------------------
// FT-142 — add-cli-subcommand. Cells (6): clap_args_module,
// handler_module, registration_wiring, mcp_tool_shim (optional),
// integration_test, help_doc_string. Discriminator: ships an
// integration_test under crates/decision-cli/tests/.
// ---------------------------------------------------------------------

fn add_cli_subcommand() -> TaskTypeDecl {
    let here = "crates/decision-cli/src/core/task_type/templates/add_cli_subcommand";
    TaskTypeDecl {
        name: "add-cli-subcommand".to_string(),
        cells: vec![
            cell("clap_args_module", "rust-source", here, "code-writer", &[]),
            cell(
                "handler_module",
                "rust-source",
                here,
                "code-writer",
                &["clap_args_module"],
            ),
            cell(
                "registration_wiring",
                "rust-source",
                here,
                "",
                &["clap_args_module", "handler_module"],
            ),
            cell(
                "mcp_tool_shim",
                "rust-source",
                here,
                "code-writer",
                &["handler_module"],
            ),
            cell(
                "integration_test",
                "rust-source",
                here,
                "code-writer",
                &["clap_args_module", "handler_module"],
            ),
            cell(
                "help_doc_string",
                "rust-source",
                here,
                "",
                &["clap_args_module"],
            ),
        ],
        coherence_audit: CoherenceAuditSpec {
            script_path: PathBuf::from("scripts/checks/cluster-audit-add-cli-subcommand.py"),
            timeout_seconds: 60,
        },
    }
}

// ---------------------------------------------------------------------
// FT-143 — extend-planner-classifier. Cells (6): inspector_trait_method,
// inspector_default_impl, inspector_production_impl, classifier_row,
// state_hash_update, unit_tests. Discriminator: touches planner.rs /
// inspect_dor.rs (not seeds.rs).
// ---------------------------------------------------------------------

fn extend_planner_classifier() -> TaskTypeDecl {
    let here = "crates/decision-cli/src/core/task_type/templates/extend_planner_classifier";
    TaskTypeDecl {
        name: "extend-planner-classifier".to_string(),
        cells: vec![
            cell("inspector_trait_method", "rust-source", here, "code-writer", &[]),
            cell(
                "inspector_default_impl",
                "rust-source",
                here,
                "",
                &["inspector_trait_method"],
            ),
            cell(
                "inspector_production_impl",
                "rust-source",
                here,
                "code-writer",
                &["inspector_trait_method"],
            ),
            cell(
                "classifier_row",
                "rust-source",
                here,
                "code-writer",
                &["inspector_trait_method", "inspector_production_impl"],
            ),
            cell(
                "state_hash_update",
                "rust-source",
                here,
                "code-writer",
                &["inspector_trait_method", "classifier_row"],
            ),
            cell(
                "unit_tests",
                "rust-source",
                here,
                "code-writer",
                &[
                    "inspector_trait_method",
                    "classifier_row",
                    "state_hash_update",
                ],
            ),
        ],
        coherence_audit: CoherenceAuditSpec {
            script_path: PathBuf::from(
                "tests/scripts/cluster-audit-extend-planner-classifier.sh",
            ),
            timeout_seconds: 60,
        },
    }
}

// ---------------------------------------------------------------------
// FT-144 — extend-role-catalog-seed. Cells (4 mandatory + 2
// conditional): iri_constants, seed_quad_function,
// init_pipeline_wiring, shacl_shape_extension, role_struct_field_extension,
// round_trip_tests. Discriminator: touches seeds.rs / role.rs (not
// planner.rs / inspect_dor.rs).
// ---------------------------------------------------------------------

fn extend_role_catalog_seed() -> TaskTypeDecl {
    let here = "crates/decision-cli/src/core/task_type/templates/extend_role_catalog_seed";
    TaskTypeDecl {
        name: "extend-role-catalog-seed".to_string(),
        cells: vec![
            cell("iri_constants", "rust-source", here, "", &[]),
            cell(
                "seed_quad_function",
                "rust-source",
                here,
                "code-writer",
                &["iri_constants"],
            ),
            cell(
                "init_pipeline_wiring",
                "rust-source",
                here,
                "",
                &["seed_quad_function"],
            ),
            cell(
                "shacl_shape_extension",
                "turtle",
                here,
                "code-writer",
                &["iri_constants", "seed_quad_function"],
            ),
            cell(
                "role_struct_field_extension",
                "rust-source",
                here,
                "code-writer",
                &["iri_constants"],
            ),
            cell(
                "round_trip_tests",
                "rust-source",
                here,
                "code-writer",
                &[
                    "seed_quad_function",
                    "init_pipeline_wiring",
                    "shacl_shape_extension",
                    "role_struct_field_extension",
                ],
            ),
        ],
        coherence_audit: CoherenceAuditSpec {
            script_path: PathBuf::from(
                "scripts/checks/cluster-audit-extend-role-catalog-seed.py",
            ),
            timeout_seconds: 60,
        },
    }
}

/// Local helper for terser registry entries. Captures the common
/// shape of a CellDecl construction so each TaskType's `vec![]`
/// stays readable.
fn cell(
    name: &str,
    artifact_type: &str,
    template_dir: &str,
    capability_id: &str,
    derived_from: &[&str],
) -> super::types::CellDecl {
    super::types::CellDecl {
        name: name.to_string(),
        artifact_type: artifact_type.to_string(),
        prompt_template_path: PathBuf::from(format!("{template_dir}/{name}.tmpl")),
        model_binding_capability_id: capability_id.to_string(),
        derived_from: derived_from.iter().map(|s| (*s).to_string()).collect(),
    }
}
