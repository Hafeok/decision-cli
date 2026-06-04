//! Type declarations for TaskType + Cell catalog (FT-139 / ADR-080).

use std::path::PathBuf;

/// A TaskType declares a typed cluster of cells that together implement
/// a known feature shape (e.g. "add-judge-worker", "add-artifact-type").
/// Sibling TaskTypes' clusters discriminate via coherence-audit checks
/// that catch misclassification.
#[derive(Debug, Clone)]
pub struct TaskTypeDecl {
    /// Stable kebab-case name matched against the feature_spec's
    /// front-matter `task_type:` field.
    pub name: String,
    /// One CellDecl per artifact the cluster emits, in any order;
    /// runtime ordering is recovered via `topo::topo_order`.
    pub cells: Vec<CellDecl>,
    /// Pointer + timeout for the cluster's coherence audit script.
    pub coherence_audit: CoherenceAuditSpec,
}

/// One cell in a cluster. Each cell emits one typed artifact via one
/// prompt + one model binding, derived from zero or more upstream
/// cells (the contract surface the cluster audits).
#[derive(Debug, Clone)]
pub struct CellDecl {
    /// Cell name (unique within its TaskType).
    pub name: String,
    /// Stable identifier for the artifact type this cell emits
    /// (informational; not yet a first-class product-cli artifact type
    /// per ADR-080 §Decision §1).
    pub artifact_type: String,
    /// Path (relative to repo root) of the prompt template.
    pub prompt_template_path: PathBuf,
    /// Stable id of the capability binding to resolve at dispatch
    /// time. Empty string for cells that do not invoke an LLM
    /// (mechanical / deterministic templates).
    pub model_binding_capability_id: String,
    /// Names of upstream cells this cell derives from. Used by
    /// `topo::topo_order` to compute dispatch order.
    pub derived_from: Vec<String>,
}

/// Pointer to the script that runs the cluster's coherence audit and
/// the wall-clock budget. The script exits 0 on pass, 1 on audit
/// failure, 2 on unrunnable. Invoked after every cell emits.
#[derive(Debug, Clone)]
pub struct CoherenceAuditSpec {
    /// Path (relative to repo root) of the audit script.
    pub script_path: PathBuf,
    /// Wall-clock budget for the audit invocation.
    pub timeout_seconds: u32,
}
