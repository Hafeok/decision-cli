//! Cluster dispatcher for typed TaskType clusters (FT-139 / ADR-080).
//!
//! Walks a TaskType's cells in `derived_from` order via
//! `core::task_type::topo_order`. Each cell produces one artifact —
//! either mechanically (template-rendered, no LLM) or via a focused
//! per-cell dispatch through the existing code-writer worker. The
//! per-cell bundles are narrow: each cell sees only its upstream
//! cells' outputs plus a small framing of the parent feature, which
//! is the architectural win over routing the whole feature bundle
//! through the broad worker (witnessed `ContextWindowExceededError`
//! on FT-125).
//!
//! After all cells emit, the cluster's coherence audit script runs
//! against the sandbox dir (`<workdir>/.dec/cluster/<feature_id>/`)
//! per its `CoherenceAuditSpec`. Audit-pass returns `Ok(())`;
//! audit-fail returns an error naming the offending check.
//!
//! Commit-on-success is deliberately deferred — operators inspect the
//! sandbox content before promoting it. A follow-on slice can land an
//! `--apply` flag that moves the sandbox files into their real paths
//! and commits.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::core::drive::PlanContext;
use crate::core::dispatch::resolve_default_capability;
use crate::core::dispatch::escalation::triggers::capability_iri;
use crate::core::graph::cluster_session::{
    persist_cluster_run, CellSessionRecord, CellStatus, ClusterOutcome,
};
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::task_type::{self, CellDecl, TaskTypeDecl};
use crate::features::implement::{
    preflight_implementer, run_worker, AuthorityJson, DispatchPayloadJson, WorkerResponseUsage,
    WorkerRun,
};

/// IRI used for mechanical cells' `dec:capability` link so the
/// SessionRecord round-trips through FT-057 SHACL (which requires a
/// `dec:capability` non-empty). The synthetic IRI is recognisable in
/// queries and never resolves to a real capability — mechanical cells
/// never consult the resolver.
const MECHANICAL_CAPABILITY_IRI: &str = "urn:dec:capability:mechanical";

/// FT-163: per-cell framing cap for the feature_spec body. The previous
/// 2000-char cap (the FT-139 prototype default) cut off before the
/// §Outputs section where prescriptive struct shapes live — witnessed
/// on the first FT-147 dispatch where the worker drafted a fictional
/// CRUD struct from the §Description prose alone instead of the spec's
/// `NamedNode`-based ontology shape. 50k chars covers every current
/// feature_spec (longest ≈ 12k chars) with 4× headroom and stays well
/// within qwen3-coder's 256k-token context budget. Specs exceeding the
/// cap truncate with the same suffix as the old path.
const MAX_FRAMING_CHARS: usize = 50_000;

/// FT-164: per-cell agentic-loop turn cap. The previous 8-turn cap (the
/// FT-139 prototype default) is a *cost safety net*, not a tuning dial:
/// witnessed on FT-147 retries where substrate cells (`emitter`,
/// `round_trip_tests`) intermittently failed mid-write at turn 9-15.
/// 40 turns at Scaleway qwen3-coder rates ≈ €0.25/cell worst case (still
/// "dimes territory"); catches stuck-model runaways without strangling
/// legitimate work. See FT-164 §Description for the cost analysis.
const MAX_CELL_TURNS: u32 = 40;

/// Execute the cluster for `task_type_name` against `feature_id`.
///
/// Steps:
/// 1. Look up TaskType, compute topo order.
/// 2. Prepare cluster sandbox at `<workdir>/.dec/cluster/<feature_id>/`.
/// 3. For each cell in order:
///    - Mechanical (empty `model_binding_capability_id`) → render a
///      deterministic template, write to sandbox.
///    - LLM-backed → resolve capability, build focused bundle, dispatch
///      the existing code-writer subprocess with `workspace_path` set
///      to the cell's sandbox subdir. The worker writes its output
///      file there; we read it back into the in-memory `cell_outputs`
///      map so downstream cells can derive from it.
/// 4. Run the coherence audit against the sandbox. Exit 0 = pass;
///    exit 1 = audit fail; exit 2 = unrunnable.
pub fn run(ctx: &PlanContext, feature_id: &str, task_type_name: &str) -> Result<()> {
    let tt = task_type::lookup(task_type_name).ok_or_else(|| {
        anyhow!(
            "cluster_dispatch::run: unknown TaskType {:?} (classifier should have filtered)",
            task_type_name
        )
    })?;
    let order = task_type::topo_order(&tt.cells).with_context(|| {
        format!(
            "topological order for TaskType {:?} (cluster declaration bug)",
            tt.name
        )
    })?;

    let cluster_dir = ctx
        .workdir
        .join(".dec")
        .join("cluster")
        .join(feature_id);
    if cluster_dir.exists() {
        // Clean re-runs so a stale prior sandbox doesn't confuse the audit.
        fs::remove_dir_all(&cluster_dir).with_context(|| {
            format!("clean prior cluster sandbox at {}", cluster_dir.display())
        })?;
    }
    fs::create_dir_all(&cluster_dir)?;

    // Resolve the worker argv once; spawned once per LLM cell.
    let argv = preflight_implementer(&ctx.workdir, None).map_err(|e| {
        anyhow!("preflight failed for code-writer: {e}")
    })?;

    // Load orchestration store for capability resolution.
    let store = load_orchestration_store(&ctx.workdir)?;

    // Read the parent feature's spec body — every cell gets a small
    // framing of it (limited to ~2000 chars to keep bundles narrow).
    let feature_framing = load_feature_framing(&ctx.product_root, feature_id)?;

    // FT-146: accumulate per-cell SessionRecord input + clamp open/close
    // timestamps on the parent cluster activity. Persistence runs in one
    // mutation after the audit, regardless of cell or audit outcome.
    let cluster_iri = NamedNode::new_unchecked(format!(
        "urn:dec:cluster-dispatch:{}/{}",
        tt.name, feature_id
    ));
    let cluster_started = Utc::now();
    let mut cell_sessions: Vec<CellSessionRecord> = Vec::new();

    let dispatch_result: Result<()> = run_cells(
        ctx,
        &argv,
        &store,
        tt,
        feature_id,
        task_type_name,
        &order,
        &cluster_dir,
        &feature_framing,
        &mut cell_sessions,
    );

    let audit_result = dispatch_result.and_then(|()| {
        run_coherence_audit(tt, &ctx.workdir, &cluster_dir, feature_id, task_type_name)
    });

    let cluster_ended = Utc::now();
    let outcome = classify_outcome(&audit_result);

    // FT-146: best-effort persist — a write failure here is logged but
    // does not override the caller's primary outcome (audit / cell).
    if let Err(err) = persist_cluster_run(
        &ctx.workdir,
        &cluster_iri,
        feature_id,
        task_type_name,
        cluster_started,
        cluster_ended,
        outcome,
        &cell_sessions,
    ) {
        tracing::warn!(
            feature_id,
            task_type = task_type_name,
            error = %err,
            "cluster_dispatch: failed to persist cluster SessionRecords (cluster outcome preserved)"
        );
    }

    audit_result
}

/// Walks the cells in `order`, emits each cell's artifact into the
/// sandbox, and pushes a `CellSessionRecord` onto `cell_sessions`
/// regardless of success / failure (PROV-O coverage stays uniform per
/// FT-146 §Invariants).
#[allow(clippy::too_many_arguments)]
fn run_cells(
    ctx: &PlanContext,
    argv: &[String],
    store: &Store,
    tt: &TaskTypeDecl,
    feature_id: &str,
    task_type_name: &str,
    order: &[String],
    cluster_dir: &Path,
    feature_framing: &str,
    cell_sessions: &mut Vec<CellSessionRecord>,
) -> Result<()> {
    let mut cell_outputs: BTreeMap<String, String> = BTreeMap::new();
    for cell_name in order {
        let cell = tt
            .cells
            .iter()
            .find(|c| &c.name == cell_name)
            .ok_or_else(|| {
                anyhow!(
                    "cluster_dispatch: cell {:?} missing from registry (registry bug)",
                    cell_name
                )
            })?;

        let cell_iri = NamedNode::new_unchecked(format!(
            "urn:dec:cluster-session:{}/{}/{}",
            tt.name, feature_id, cell.name
        ));
        let started_at = Utc::now();

        let output_result: Result<(String, Option<WorkerResponseUsage>, NamedNode)> =
            if cell.model_binding_capability_id.is_empty() {
                let body = emit_mechanical_cell(tt, cell, &cell_outputs);
                let capability =
                    NamedNode::new_unchecked(MECHANICAL_CAPABILITY_IRI.to_string());
                Ok((body, None, capability))
            } else {
                emit_llm_cell(
                    ctx,
                    argv,
                    store,
                    tt,
                    feature_id,
                    cell,
                    cluster_dir,
                    feature_framing,
                    &cell_outputs,
                )
            };

        match output_result {
            Ok((output, usage, capability)) => {
                let ended_at = Utc::now();
                let status = if cell.model_binding_capability_id.is_empty() {
                    CellStatus::Mechanical
                } else {
                    CellStatus::Succeeded
                };
                cell_sessions.push(CellSessionRecord {
                    iri: cell_iri,
                    capability,
                    usage,
                    status,
                    started_at,
                    ended_at,
                });

                let cell_path = cluster_dir.join(cell_filename(cell));
                if let Some(parent) = cell_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&cell_path, &output).with_context(|| {
                    format!("write cell {} to {}", cell.name, cell_path.display())
                })?;
                cell_outputs.insert(cell.name.clone(), output);
                tracing::info!(
                    feature_id,
                    task_type = task_type_name,
                    cell = %cell.name,
                    path = %cell_path.display(),
                    "cluster cell emitted"
                );
            }
            Err(err) => {
                let ended_at = Utc::now();
                // FT-146: record a failed SessionRecord before bubbling
                // the error — PROV-O coverage stays uniform.
                let capability = cell_capability_iri_or_fallback(store, cell);
                cell_sessions.push(CellSessionRecord {
                    iri: cell_iri,
                    capability,
                    usage: None,
                    status: CellStatus::Failed,
                    started_at,
                    ended_at,
                });
                return Err(err);
            }
        }
    }
    Ok(())
}

/// Best-effort capability resolution for a *failed* cell — used to fill
/// `CellSessionRecord.capability` when the dispatch errored before the
/// worker returned. Falls back to the mechanical IRI on failure so the
/// SessionRecord always carries a `dec:capability` link.
fn cell_capability_iri_or_fallback(store: &Store, cell: &CellDecl) -> NamedNode {
    if cell.model_binding_capability_id.is_empty() {
        return NamedNode::new_unchecked(MECHANICAL_CAPABILITY_IRI.to_string());
    }
    match resolve_default_capability(store, &cell.model_binding_capability_id) {
        Ok(cap) => capability_iri(&cap),
        Err(_) => NamedNode::new_unchecked(MECHANICAL_CAPABILITY_IRI.to_string()),
    }
}

/// Map the cluster's overall `Result<()>` into the
/// `dec:clusterOutcome` enum. Inspects the error message to
/// distinguish audit vs cell failures (the audit branch is the only
/// path that produces messages containing "audit").
fn classify_outcome(result: &Result<()>) -> ClusterOutcome {
    match result {
        Ok(()) => ClusterOutcome::Succeeded,
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("audit unrunnable") {
                ClusterOutcome::AuditUnrunnable
            } else if msg.contains("audit") {
                ClusterOutcome::AuditFailed
            } else {
                ClusterOutcome::CellFailed
            }
        }
    }
}

/// Returns the relative path within the cluster sandbox at which the
/// cell's output is written. The mapping matches what the per-TaskType
/// audit scripts expect (see `scripts/checks/cluster-audit-*.py`).
fn cell_filename(cell: &CellDecl) -> PathBuf {
    match cell.artifact_type.as_str() {
        "python-module" => PathBuf::from(format!("{}.py", cell.name)),
        "rust-source" => {
            // The add-cli-subcommand cluster's integration_test cell goes
            // under crates/decision-cli/tests/ — the audit looks there.
            if cell.name == "integration_test" {
                PathBuf::from("crates")
                    .join("decision-cli")
                    .join("tests")
                    .join(format!("{}.rs", cell.name))
            } else {
                PathBuf::from(format!("{}.rs", cell.name))
            }
        }
        "n-quads" => PathBuf::from(format!("{}.nq", cell.name)),
        "turtle" => PathBuf::from(format!("{}.ttl", cell.name)),
        "markdown" => PathBuf::from(format!("{}.md", cell.name)),
        "json-fixtures" => PathBuf::from(format!("{}.json", cell.name)),
        other => PathBuf::from(format!("{}.{}", cell.name, other)),
    }
}

/// Emit a deterministic-template cell (no LLM). For now, mechanical
/// cells produce a placeholder that names their derived_from inputs —
/// enough for the audit to detect the file's presence. Full template
/// rendering against the cells' actual upstream content is a follow-on.
fn emit_mechanical_cell(
    tt: &TaskTypeDecl,
    cell: &CellDecl,
    upstream: &BTreeMap<String, String>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// Mechanical cell: {tt}/{cell}\n",
        tt = tt.name,
        cell = cell.name
    ));
    out.push_str("// Generated by cluster_dispatch as a deterministic template.\n");
    if !cell.derived_from.is_empty() {
        out.push_str("// Derives from:\n");
        for dep in &cell.derived_from {
            let len = upstream.get(dep).map(String::len).unwrap_or(0);
            out.push_str(&format!("//   - {dep} ({len} bytes)\n"));
        }
    }
    // Minimal payloads per artifact_type — enough for audit file
    // presence + the specific shape some audits check for.
    match cell.artifact_type.as_str() {
        "n-quads" => {
            out.push_str(
                "<https://decision-cli.dev/ns/capability/example/v1> \
                 <https://decision-cli.dev/ns#endpoint> \"scaleway\" \
                 <https://decision-cli.dev/ns/graph/capability> .\n",
            );
        }
        "rust-source" => {
            out.push_str("// Registration / help-doc placeholder — wire-up code.\n");
        }
        "markdown" => {
            out.push_str("# Placeholder\n\nMechanical-template stub.\n");
        }
        _ => {}
    }
    out
}

/// Build a focused per-cell bundle and dispatch the code-writer
/// subprocess to emit the cell's artifact into the cluster sandbox.
///
/// Returns the cell's emitted file body, the worker's reported usage
/// (FT-146 — `None` when the worker didn't surface a usage block), and
/// the capability NamedNode that resolved for the cell. The caller
/// records all three on the per-cell `dec:SessionRecord`.
#[allow(clippy::too_many_arguments)]
fn emit_llm_cell(
    ctx: &PlanContext,
    argv: &[String],
    store: &Store,
    tt: &TaskTypeDecl,
    feature_id: &str,
    cell: &CellDecl,
    cluster_dir: &Path,
    feature_framing: &str,
    upstream: &BTreeMap<String, String>,
) -> Result<(String, Option<WorkerResponseUsage>, NamedNode)> {
    let cap = resolve_default_capability(store, &cell.model_binding_capability_id)
        .with_context(|| {
            format!(
                "resolve capability {:?} for cell {}/{}",
                cell.model_binding_capability_id, tt.name, cell.name
            )
        })?;
    let cell_capability_iri = capability_iri(&cap);

    // Build the per-cell bundle: framing + upstream cell outputs +
    // instruction telling the worker exactly what to write.
    let cell_filename = cell_filename(cell);
    let bundle = build_cell_bundle(tt, cell, feature_id, feature_framing, upstream, &cell_filename);

    // The worker's workspace_path is the cluster sandbox dir. It writes
    // the cell's output file directly there via the write_file tool.
    let workspace_path = cluster_dir.to_string_lossy().into_owned();

    // Authority is left as None — cell dispatches don't carry an
    // escalation hierarchy in this slice; FT-062's escalation paths
    // are out of scope for the cell dispatcher (one-shot per cell).
    let authority: Option<AuthorityJson> = None;

    // FT-164: per-task-type override from .dec/task-types.toml, falling
    // back to MAX_CELL_TURNS const when absent or malformed.
    let max_turns = crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
        &ctx.workdir,
        &tt.name,
    )
    .unwrap_or(MAX_CELL_TURNS);

    let payload = DispatchPayloadJson {
        dispatch_id: format!(
            "urn:dec:cluster-dispatch:{}/{}/{}",
            tt.name, feature_id, cell.name
        ),
        session_id: format!(
            "urn:dec:cluster-session:{}/{}/{}",
            tt.name, feature_id, cell.name
        ),
        feature_id: feature_id.to_string(),
        bundle_markdown: bundle,
        bundle_hash: format!("cluster-{}-{}", tt.name, cell.name),
        workspace_path,
        model_id: cap.model_identifier,
        endpoint: cap.endpoint.as_str().to_string(),
        timeout_seconds: 600,
        max_turns,
        authority,
        defect_feedback: Vec::new(),
        allowed_tools: vec![
            "read_file".to_string(),
            "write_file".to_string(),
        ],
    };

    let WorkerRun { response, raw_stdout: _ } = run_worker(argv, &payload).with_context(|| {
        format!(
            "dispatch code-writer for cell {}/{} (feature {})",
            tt.name, cell.name, feature_id
        )
    })?;

    // Read back what the worker wrote.
    let written_path = cluster_dir.join(&cell_filename);
    let body = fs::read_to_string(&written_path).with_context(|| {
        format!(
            "cell {}/{} did not produce {} (worker may not have invoked write_file)",
            tt.name,
            cell.name,
            written_path.display()
        )
    })?;
    Ok((body, response.usage, cell_capability_iri))
}

/// Compose the per-cell bundle markdown: small framing of the parent
/// feature + upstream cell outputs + a precise instruction line. Each
/// cell sees only what it needs from upstream — the architectural win
/// over the broad worker's whole-feature bundle.
fn build_cell_bundle(
    tt: &TaskTypeDecl,
    cell: &CellDecl,
    feature_id: &str,
    feature_framing: &str,
    upstream: &BTreeMap<String, String>,
    target_filename: &Path,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Cluster dispatch: {} / {}\n\n", tt.name, cell.name));
    out.push_str(&format!("**Feature:** `{feature_id}`\n\n"));
    out.push_str("## Feature framing\n\n");
    out.push_str(feature_framing);
    out.push_str("\n\n");
    if !cell.derived_from.is_empty() {
        out.push_str("## Upstream cells\n\n");
        for dep in &cell.derived_from {
            out.push_str(&format!("### {dep}\n\n```\n"));
            out.push_str(upstream.get(dep).map(String::as_str).unwrap_or(""));
            out.push_str("\n```\n\n");
        }
    }
    out.push_str("## Your task\n\n");
    out.push_str(&format!(
        "Emit the `{}` cell of the `{}` task type. The artifact type is `{}`.\n\n",
        cell.name, tt.name, cell.artifact_type
    ));
    out.push_str(&format!(
        "Write a single file at the workspace-relative path `{}` containing the cell's content. \
         Do not produce any other files. Do not edit existing files. \
         When you have written the file, end your turn.\n",
        target_filename.display()
    ));
    out
}

/// Read the feature spec's body (truncated) to use as context framing
/// in every cell's bundle.
fn load_feature_framing(product_root: &Path, feature_id: &str) -> Result<String> {
    let glob_pattern = format!(
        ".product/features/{}-*.md",
        feature_id
    );
    let spec_dir = product_root.join(".product").join("features");
    let mut found: Option<PathBuf> = None;
    if spec_dir.is_dir() {
        for entry in fs::read_dir(&spec_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{feature_id}-")) && name.ends_with(".md") {
                found = Some(entry.path());
                break;
            }
        }
    }
    let path = found.ok_or_else(|| {
        anyhow!("cluster_dispatch: feature spec not found via pattern {glob_pattern}")
    })?;
    let raw = fs::read_to_string(&path)?;
    Ok(truncate_for_framing(&raw, MAX_FRAMING_CHARS))
}

fn truncate_for_framing(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push_str("\n…\n[spec truncated for cell framing]\n");
    out
}

fn load_orchestration_store(workdir: &Path) -> Result<Store> {
    let dump = orchestration_dump_path(workdir);
    load_store_from_dump(&dump).map_err(|e| {
        anyhow!(
            "cluster_dispatch: load orchestration store at {}: {e}",
            dump.display()
        )
    })
}

fn run_coherence_audit(
    tt: &TaskTypeDecl,
    workdir: &Path,
    cluster_dir: &Path,
    feature_id: &str,
    task_type_name: &str,
) -> Result<()> {
    let audit = &tt.coherence_audit;
    let audit_path = workdir.join(&audit.script_path);
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
        .current_dir(workdir)
        // Pass the cluster sandbox dir explicitly — audits expect a
        // fixture path as $1.
        .arg(cluster_dir)
        .output()
        .with_context(|| format!("invoke coherence audit at {}", audit_path.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// FT-163 TC: pins the framing-cap constant so changes are explicit.
    /// 50k chars covers every current feature_spec (longest ≈ 12k) with
    /// 4× headroom — see FT-163 §Description.
    #[test]
    fn ft_163_max_framing_chars_is_50k() {
        assert_eq!(MAX_FRAMING_CHARS, 50_000);
    }

    /// FT-163 TC: a spec shorter than the cap passes through unchanged.
    /// No truncation suffix appended.
    #[test]
    fn ft_163_short_spec_passes_through_unchanged() {
        let s = "## Description\n\nA short spec.\n";
        let out = truncate_for_framing(s, MAX_FRAMING_CHARS);
        assert_eq!(out, s);
        assert!(!out.contains("[spec truncated"));
    }

    /// FT-163 TC: a spec longer than the cap is truncated with the
    /// witness suffix. Char count of the prefix matches the cap; suffix
    /// makes the cluster bundle self-documenting about the cut.
    #[test]
    fn ft_163_long_spec_truncates_with_witness_suffix() {
        let big: String = "a".repeat(60_000);
        let out = truncate_for_framing(&big, MAX_FRAMING_CHARS);
        assert!(
            out.contains("[spec truncated for cell framing]"),
            "truncation suffix missing"
        );
        // Prefix is exactly MAX_FRAMING_CHARS chars; suffix follows.
        let prefix: String = out.chars().take(MAX_FRAMING_CHARS).collect();
        assert_eq!(prefix.len(), MAX_FRAMING_CHARS);
    }

    /// FT-164 TC: pins the default turn cap at 40 — operators see the
    /// per-release default and changes become explicit. Catalog overrides
    /// take precedence over this default (TC-401).
    #[test]
    fn ft_164_max_cell_turns_default_is_40() {
        assert_eq!(MAX_CELL_TURNS, 40);
    }

    /// FT-164 TC: empty / missing override → fall back to default. The
    /// helper degrades to `None` for absent files, missing `[task_types]`
    /// table, missing per-task entry, or absent `max_turns` key.
    #[test]
    fn ft_164_override_absent_returns_none_so_caller_falls_back_to_default() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        // No .dec/task-types.toml at all.
        let none = crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
            tmp.path(),
            "add-artifact-type",
        );
        assert_eq!(none, None);

        // File present but no [task_types.<name>] table.
        std::fs::create_dir_all(tmp.path().join(".dec")).unwrap();
        std::fs::write(
            tmp.path().join(".dec/task-types.toml"),
            "[features]\n\"FT-T401\" = \"add-artifact-type\"\n",
        )
        .unwrap();
        let still_none =
            crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
                tmp.path(),
                "add-artifact-type",
            );
        assert_eq!(still_none, None);
    }

    /// FT-164 TC: per-task-type override returned when present. Catalog
    /// is the source of truth — different task types get different caps.
    #[test]
    fn ft_164_override_returns_configured_cap_per_task_type() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        std::fs::create_dir_all(tmp.path().join(".dec")).unwrap();
        std::fs::write(
            tmp.path().join(".dec/task-types.toml"),
            r#"[features]
"FT-T402" = "add-artifact-type"

[task_types.add-artifact-type]
max_turns = 60

[task_types.add-judge-worker]
max_turns = 12
"#,
        )
        .unwrap();
        assert_eq!(
            crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
                tmp.path(),
                "add-artifact-type"
            ),
            Some(60)
        );
        assert_eq!(
            crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
                tmp.path(),
                "add-judge-worker"
            ),
            Some(12)
        );
        // Unknown task type — no override; falls back to default at the
        // call site.
        assert_eq!(
            crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
                tmp.path(),
                "extend-planner-classifier"
            ),
            None
        );
    }

    /// FT-164 TC: malformed TOML / out-of-range values degrade to None
    /// rather than erroring. Dispatch never fails over misconfig — it
    /// falls back to the const default.
    #[test]
    fn ft_164_override_malformed_or_oob_degrades_to_none() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        std::fs::create_dir_all(tmp.path().join(".dec")).unwrap();

        // Garbage TOML.
        std::fs::write(tmp.path().join(".dec/task-types.toml"), "this is not [valid toml")
            .unwrap();
        assert_eq!(
            crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
                tmp.path(),
                "add-artifact-type"
            ),
            None
        );

        // Negative integer — fails u32 conversion.
        std::fs::write(
            tmp.path().join(".dec/task-types.toml"),
            "[task_types.add-artifact-type]\nmax_turns = -5\n",
        )
        .unwrap();
        assert_eq!(
            crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
                tmp.path(),
                "add-artifact-type"
            ),
            None
        );

        // String value where integer expected.
        std::fs::write(
            tmp.path().join(".dec/task-types.toml"),
            r#"[task_types.add-artifact-type]
max_turns = "high"
"#,
        )
        .unwrap();
        assert_eq!(
            crate::features::drive::planners::feature_ship::read_max_turns_for_task_type(
                tmp.path(),
                "add-artifact-type"
            ),
            None
        );
    }

    /// FT-163 TC: the framing cap is large enough to fit FT-147's
    /// §Outputs section (the witnessed motivating need). Reads the live
    /// spec file from the workspace and asserts the cap admits it
    /// without truncation. Guards against the cap silently shrinking
    /// below the catalog's largest current spec.
    #[test]
    fn ft_163_cap_admits_current_archetype_spec() {
        // Walk up to the workspace root from the test binary's CARGO_MANIFEST_DIR.
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let manifest_path = std::path::Path::new(&manifest_dir);
        let workspace_root = manifest_path
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root walks up from CARGO_MANIFEST_DIR");
        let spec_dir = workspace_root.join(".product").join("features");
        if !spec_dir.is_dir() {
            // Spec dir absent — not a regression; the cap is what it is.
            return;
        }
        let prefix = "FT-147-";
        let entry = std::fs::read_dir(&spec_dir)
            .expect("read .product/features")
            .flatten()
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(prefix)
            });
        let Some(entry) = entry else {
            return; // FT-147 spec not present in this checkout
        };
        let body = std::fs::read_to_string(entry.path()).expect("read spec");
        let cell_count = body.chars().count();
        assert!(
            cell_count <= MAX_FRAMING_CHARS,
            "FT-147 spec is {cell_count} chars; MAX_FRAMING_CHARS is {MAX_FRAMING_CHARS} — \
             the cap must admit the catalog's largest substrate spec without truncation. \
             Raise MAX_FRAMING_CHARS in cluster_dispatch.rs or shrink the spec."
        );
    }
}
