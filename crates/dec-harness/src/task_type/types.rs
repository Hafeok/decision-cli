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
    /// FT-166: parameters cells may reference in their `output_path` via
    /// `{name}` placeholders. Per-feature values land in
    /// `.dec/task-types.toml` under `[parameters."<feature_id>"]`.
    /// Empty list ≡ no parameters; cluster falls back to FT-139 flat
    /// convention regardless of feature.
    pub parameters: Vec<TaskTypeParameter>,
    /// FT-178: fixed crate-contract text rendered into every LLM cell
    /// bundle — names the target crate, its allowed dependency universe,
    /// and its forbidden crates. Empty ≡ none.
    pub crate_contract: String,
    /// FT-178: repo-relative files whose distilled public surface is
    /// appended to every LLM cell bundle ("existing crate interfaces").
    pub context_files: Vec<PathBuf>,
}

/// FT-166: a parameter a TaskType's cells can interpolate into their
/// `output_path`. Per-feature values land in `.dec/task-types.toml` under
/// `[parameters."<feature_id>"]`. A parameter with no default value is
/// required — dispatch fails fast when a feature does not supply it.
#[derive(Debug, Clone)]
pub struct TaskTypeParameter {
    /// Snake-case identifier the cell references via `{name}` in its
    /// `output_path`.
    pub name: String,
    /// Operator-facing description surfaced in dispatch errors when the
    /// parameter is missing.
    pub description: String,
    /// Default value applied when no per-feature override is configured.
    /// `None` ≡ required parameter.
    pub default: Option<String>,
}

/// FT-177: per-cell feature-spec framing contract. Hallucination means
/// the context was too big or unspecific — only the cell that
/// transcribes the spec's prescribed shape sees spec prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellFraming {
    /// The spec's `### Outputs` section (fallback: capped full body).
    /// Default — pre-FT-177 TaskTypes keep today's behaviour.
    #[default]
    SpecOutputs,
    /// One line of feature identity; no spec body at all.
    Minimal,
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
    /// FT-177: how much feature-spec framing this cell's bundle carries.
    pub framing: CellFraming,
    /// FT-177: when true, upstream `.rs` cell outputs are distilled to
    /// their public surface (SPMC) instead of arriving whole. Turtle
    /// upstreams always arrive whole.
    pub distill_upstream: bool,
    /// Names of upstream cells this cell derives from. Used by
    /// `topo::topo_order` to compute dispatch order.
    pub derived_from: Vec<String>,
    /// FT-166: workspace-relative output path with optional `{parameter}`
    /// placeholders resolved at dispatch time. Empty path → fall back to
    /// the FT-139 flat convention `<cell_name>.<ext>` via
    /// `cluster_dispatch::cell_filename`.
    ///
    /// Examples:
    /// - `"crates/dec-ontology/src/ontology/{artifact_name}.rs"`
    /// - `"workers/{worker_name}/src/{worker_name}/agent/loop.py"`
    /// - `""` (uses flat convention — backwards-compat with FT-145's
    ///   `add-cli-subcommand` cluster).
    pub output_path: PathBuf,
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
