//! Cluster dispatcher for typed TaskType clusters (FT-139 / ADR-080).
//!
//! Walks a TaskType's cells in `derived_from` order via
//! `core::task_type::topo_order`, emits each cell's artifact (stub
//! emission in this slice; full per-cell LLM dispatch is a follow-on
//! once the broad worker's rate-limit budget allows it), then runs
//! the coherence audit. The audit's exit-0 / exit-1 / exit-2 codes
//! mirror the TC runner contract from ADR-013 (pass / fail /
//! unrunnable).
//!
//! For the substrate slice (FT-139 §Phase 2), `run` is a real-but-
//! minimal driver: it looks up the TaskType, computes topo order,
//! invokes the audit script if it exists, and returns Ok(()) when
//! the cluster machinery is wired correctly. Per-cell LLM-backed
//! emission lives behind a feature flag — operators run the broad
//! worker as the escape hatch (ADR-080) until that lands.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::Command;

use crate::core::drive::PlanContext;
use crate::core::task_type;

/// Execute the cluster for `task_type_name` against `feature_id`.
///
/// Substrate-slice behaviour:
/// 1. Look up the TaskType. Unknown TaskType is a hard error (the
///    classifier already validated; reaching here with an unknown
///    name is a programming bug).
/// 2. Compute `topo_order(cells)`. Cycle / missing-dep is a hard
///    error (TaskType declarations are static; same reasoning).
/// 3. Invoke the audit script at `coherence_audit.script_path`
///    against `ctx.workdir`. Exit 0 = pass; exit 1 = fail; exit 2 =
///    unrunnable. Absent script is treated as "audit deferred" and
///    succeeds (a clean signal for the operator that the cluster's
///    audit prototype hasn't shipped yet).
pub fn run(ctx: &PlanContext, feature_id: &str, task_type_name: &str) -> Result<()> {
    let tt = task_type::lookup(task_type_name).ok_or_else(|| {
        anyhow!(
            "cluster_dispatch::run: unknown TaskType {:?} (classifier should have filtered)",
            task_type_name
        )
    })?;

    let _order = task_type::topo_order(&tt.cells).with_context(|| {
        format!(
            "topological order for TaskType {:?} (cluster declaration bug)",
            tt.name
        )
    })?;

    let audit = &tt.coherence_audit;
    let audit_path = ctx.workdir.join(&audit.script_path);
    if !audit_path.exists() {
        tracing::warn!(
            feature_id,
            task_type = task_type_name,
            script = %audit.script_path.display(),
            "cluster_dispatch: audit script not present; treating as deferred"
        );
        return Ok(());
    }

    let output = Command::new(&audit_path)
        .current_dir(&ctx.workdir)
        .arg(feature_id)
        .output()
        .with_context(|| {
            format!(
                "invoke coherence audit at {p}",
                p = audit_path.display()
            )
        })?;

    match output.status.code() {
        Some(0) => Ok(()),
        Some(1) => Err(anyhow!(
            "cluster_dispatch::run: audit failed for TaskType {:?} on {}: {}",
            task_type_name,
            feature_id,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Some(2) => Err(anyhow!(
            "cluster_dispatch::run: audit unrunnable for TaskType {:?} on {}: {}",
            task_type_name,
            feature_id,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        other => Err(anyhow!(
            "cluster_dispatch::run: audit exited with unexpected status {:?} (stderr: {})",
            other,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

/// Convenience that returns the cluster's topo order so callers can
/// inspect the planned dispatch sequence without executing.
#[must_use]
#[allow(dead_code)]
pub fn planned_cell_order(task_type_name: &str) -> Option<Vec<String>> {
    let tt = task_type::lookup(task_type_name)?;
    task_type::topo_order(&tt.cells).ok()
}

#[allow(dead_code)]
fn _silence_unused_path_import_warning(_p: &Path) {}
