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

use crate::core::cluster_session::{
    persist_cluster_run, CellSessionRecord, CellStatus, ClusterOutcome,
};
use crate::core::dispatch::escalation::triggers::capability_iri;
use crate::core::dispatch::resolve_default_capability;
use crate::core::drive::PlanContext;
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

/// FT-171: audit-repair rounds per cluster run. Each round re-dispatches
/// only the cells implicated by the failed audit checks against the
/// preserved sandbox; after the cap the cluster fails and the drive's
/// outer loop decides on a full re-run.
const MAX_AUDIT_REPAIR_ROUNDS: usize = 2;

/// FT-171: per-cell retry cap within one cluster run.
const MAX_CELL_RETRIES: u32 = 2;

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

    let cluster_dir = ctx.workdir.join(".dec").join("cluster").join(feature_id);
    if cluster_dir.exists() {
        // Clean re-runs so a stale prior sandbox doesn't confuse the audit.
        fs::remove_dir_all(&cluster_dir)
            .with_context(|| format!("clean prior cluster sandbox at {}", cluster_dir.display()))?;
    }
    fs::create_dir_all(&cluster_dir)?;

    // Resolve the worker argv once; spawned once per LLM cell.
    let argv = preflight_implementer(&ctx.workdir, None)
        .map_err(|e| anyhow!("preflight failed for code-writer: {e}"))?;

    // Load orchestration store for capability resolution.
    let store = load_orchestration_store(&ctx.workdir)?;

    // Read the parent feature's spec body — every cell gets a small
    // framing of it (limited to ~2000 chars to keep bundles narrow).
    let feature_framing = load_feature_framing(&ctx.product_root, feature_id)?;

    // FT-166: resolve per-feature parameters before any cell dispatches.
    // Required parameters with no default + no override fail fast here
    // with a clean operator diagnostic — saves a botched cluster dispatch.
    let params = resolve_parameters(tt, &ctx.workdir, feature_id)?;

    // FT-146: accumulate per-cell SessionRecord input + clamp open/close
    // timestamps on the parent cluster activity. Persistence runs in one
    // mutation after the audit, regardless of cell or audit outcome.
    let cluster_iri = NamedNode::new_unchecked(format!(
        "urn:dec:cluster-dispatch:{}/{}",
        tt.name, feature_id
    ));
    let cluster_started = Utc::now();
    let mut cell_sessions: Vec<CellSessionRecord> = Vec::new();

    let mut cell_outputs: BTreeMap<String, String> = BTreeMap::new();
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
        &params,
        &mut cell_sessions,
        &mut cell_outputs,
        None,
        None,
    );

    // FT-171: audit with per-cell repair — audit failure preserves the
    // sandbox and re-dispatches only the implicated cells (with the
    // audit diagnostic as corrective context) before re-auditing.
    let audit_result = dispatch_result.and_then(|()| {
        audit_with_repair(
            ctx,
            &argv,
            &store,
            tt,
            feature_id,
            task_type_name,
            &order,
            &cluster_dir,
            &feature_framing,
            &params,
            &mut cell_sessions,
            &mut cell_outputs,
        )
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
    params: &BTreeMap<String, String>,
    cell_sessions: &mut Vec<CellSessionRecord>,
    cell_outputs: &mut BTreeMap<String, String>,
    only: Option<&std::collections::BTreeSet<String>>,
    audit_context: Option<&str>,
) -> Result<()> {
    // FT-171: when repairing, `active` starts as the implicated set and
    // grows with the dependents of any retried cell whose output changed
    // (their bundles consumed the stale upstream).
    let mut active: std::collections::BTreeSet<String> = only
        .cloned()
        .unwrap_or_else(|| order.iter().cloned().collect());
    for cell_name in order {
        if !active.contains(cell_name) {
            continue;
        }
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

        // FT-166: resolve the cell's output path within the sandbox via
        // {parameter} substitution. Cells with empty output_path fall
        // back to the FT-139 flat-path convention.
        let resolved_cell_path = resolve_cell_output_path(cell, params)?;

        // FT-171: a retried cell's prior output is removed so FT-170's
        // snapshot diff sees the worker's fresh write.
        if only.is_some() {
            let prior = cluster_dir.join(&resolved_cell_path);
            if prior.exists() {
                fs::remove_file(&prior).with_context(|| {
                    format!("remove prior output of retried cell at {}", prior.display())
                })?;
            }
        }

        let cell_iri = NamedNode::new_unchecked(format!(
            "urn:dec:cluster-session:{}/{}/{}",
            tt.name, feature_id, cell.name
        ));
        let started_at = Utc::now();

        let output_result: Result<(String, Option<WorkerResponseUsage>, NamedNode)> =
            if cell.model_binding_capability_id.is_empty() {
                let body = emit_mechanical_cell(tt, cell, cell_outputs);
                let capability = NamedNode::new_unchecked(MECHANICAL_CAPABILITY_IRI.to_string());
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
                    cell_outputs,
                    &resolved_cell_path,
                    audit_context,
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

                let cell_path = cluster_dir.join(&resolved_cell_path);
                if let Some(parent) = cell_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&cell_path, &output).with_context(|| {
                    format!("write cell {} to {}", cell.name, cell_path.display())
                })?;
                // FT-171: a retried cell whose content changed staleness
                // its dependents — their bundles consumed the old output.
                let changed = cell_outputs
                    .get(&cell.name)
                    .is_some_and(|prior| prior != &output);
                if only.is_some() && changed {
                    for dep in dependents_of(tt, &cell.name) {
                        active.insert(dep);
                    }
                }
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

/// FT-166: resolve the per-feature parameter map. Merges
/// `.dec/task-types.toml [parameters."<feature_id>"]` values with the
/// TaskType-declared defaults. Returns an error naming the missing
/// parameter when a required parameter (no default) is unset — operator
/// gets a clean diagnostic before any cell dispatch.
fn resolve_parameters(
    tt: &TaskTypeDecl,
    workdir: &Path,
    feature_id: &str,
) -> Result<BTreeMap<String, String>> {
    let mut resolved = crate::features::drive::planners::feature_ship::read_parameters_for_feature(
        workdir, feature_id,
    );
    for param in &tt.parameters {
        if !resolved.contains_key(&param.name) {
            match &param.default {
                Some(default) => {
                    resolved.insert(param.name.clone(), default.clone());
                }
                None => {
                    return Err(anyhow!(
                        "cluster_dispatch: task type {:?} requires parameter `{}` ({}) for {}. \
                         Set it in .dec/task-types.toml:\n  [parameters.\"{feature_id}\"]\n  {} = \"<value>\"",
                        tt.name,
                        param.name,
                        param.description,
                        feature_id,
                        param.name,
                    ));
                }
            }
        }
    }
    Ok(resolved)
}

/// FT-166: substitute `{name}` placeholders in `template` against the
/// resolved parameter map. Literal — no escaping, no nesting. Used by
/// `resolve_cell_output_path`.
fn substitute_params(template: &str, params: &BTreeMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in params {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// FT-166: resolve a cell's output path within the sandbox. When the
/// cell declares an `output_path`, substitute parameters and return.
/// When empty, fall back to the FT-139 flat-path convention via
/// `cell_filename`. Rejects paths containing `..` after substitution
/// (sandbox containment guard).
fn resolve_cell_output_path(cell: &CellDecl, params: &BTreeMap<String, String>) -> Result<PathBuf> {
    if cell.output_path.as_os_str().is_empty() {
        return Ok(cell_filename(cell));
    }
    let template = cell.output_path.to_string_lossy().into_owned();
    let resolved = substitute_params(&template, params);
    if resolved.split('/').any(|seg| seg == "..") {
        return Err(anyhow!(
            "cluster_dispatch: cell {:?} resolved output_path contains `..` segment (sandbox containment guard): {:?}",
            cell.name,
            resolved
        ));
    }
    Ok(PathBuf::from(resolved))
}

/// FT-170: collect every file currently under `root`, as paths relative
/// to `root`. The before/after diff of two snapshots identifies exactly
/// the files one cell's worker created.
fn snapshot_files(root: &Path) -> Result<std::collections::BTreeSet<PathBuf>> {
    fn walk(root: &Path, dir: &Path, acc: &mut std::collections::BTreeSet<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                walk(root, &path, acc)?;
            } else if let Ok(rel) = path.strip_prefix(root) {
                acc.insert(rel.to_path_buf());
            }
        }
        Ok(())
    }
    let mut acc = std::collections::BTreeSet::new();
    if root.exists() {
        walk(root, root, &mut acc)?;
    }
    Ok(acc)
}

/// FT-170: deterministic cell-output placement. The harness resolved
/// `output_path` before dispatch; the worker's chosen `write_file` path
/// is advisory. After the worker returns, the cell's primary artifact is
/// guaranteed to sit at the resolved path or the cell fails loudly:
///
/// 1. Worker wrote the resolved path → no-op (stray extras tolerated).
/// 2. Worker wrote exactly one new file of the right kind elsewhere →
///    relocated to the resolved path, with the drift logged (the signal
///    feeds prompt tuning).
/// 3. Worker wrote nothing of the right kind → fail naming the expected
///    path (previously this surfaced later, at read-back or audit time).
/// 4. Multiple candidates and no resolved file → fail listing them;
///    ambiguity is never silently resolved.
///
/// The relocation refuses to overwrite a file that existed before the
/// cell ran — a prior cell's placed output is never clobbered.
fn place_cell_output(
    cluster_dir: &Path,
    resolved_rel: &Path,
    before: &std::collections::BTreeSet<PathBuf>,
    cell_label: &str,
) -> Result<()> {
    let after = snapshot_files(cluster_dir)?;
    let new_files: Vec<&PathBuf> = after.difference(before).collect();

    // Case 1 — the worker honoured the resolved path.
    if new_files.iter().any(|p| p.as_path() == resolved_rel) {
        return Ok(());
    }

    let wanted_ext = resolved_rel.extension();
    let candidates: Vec<&PathBuf> = new_files
        .iter()
        .copied()
        .filter(|p| p.extension() == wanted_ext)
        .collect();

    match candidates.as_slice() {
        // Case 3 — nothing of the right kind was produced.
        [] => Err(anyhow!(
            "cluster_dispatch: cell {cell_label} produced no {} file; expected {} \
             (worker may not have invoked write_file)",
            wanted_ext
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_else(|| "output".to_string()),
            resolved_rel.display(),
        )),
        // Case 2 — exactly one stray: the harness owns placement. The
        // resolved path is THIS cell's declared slot (the registry
        // guarantees distinct paths per cell), so any pre-existing
        // content there is this cell's own stale prior attempt —
        // witnessed on the first hardened FT-148 run, where a killed
        // worker's partial output blocked its replacement. Replace it.
        [stray] => {
            let target = cluster_dir.join(resolved_rel);
            if before.contains(resolved_rel) {
                tracing::info!(
                    cell = cell_label,
                    path = %resolved_rel.display(),
                    "replacing the cell's stale prior output at its resolved slot (FT-170)"
                );
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(cluster_dir.join(stray), &target)
                .with_context(|| format!("relocate {} to {}", stray.display(), target.display()))?;
            tracing::info!(
                cell = cell_label,
                from = %stray.display(),
                to = %resolved_rel.display(),
                "cluster cell output relocated to resolved output_path (FT-170)"
            );
            Ok(())
        }
        // Case 4 — ambiguous.
        many => Err(anyhow!(
            "cluster_dispatch: cell {cell_label} wrote {} candidate files but none at the \
             resolved path {}; refusing to guess: {}",
            many.len(),
            resolved_rel.display(),
            many.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        )),
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
    resolved_cell_path: &Path,
    audit_context: Option<&str>,
) -> Result<(String, Option<WorkerResponseUsage>, NamedNode)> {
    let cap = resolve_default_capability(store, &cell.model_binding_capability_id).with_context(
        || {
            format!(
                "resolve capability {:?} for cell {}/{}",
                cell.model_binding_capability_id, tt.name, cell.name
            )
        },
    )?;
    let cell_capability_iri = capability_iri(&cap);

    // Build the per-cell bundle: framing + upstream cell outputs +
    // instruction telling the worker exactly what to write (FT-166: the
    // resolved path may include subdirectories — the bundle surfaces it
    // verbatim and the worker creates intermediate dirs via write_file).
    let bundle = build_cell_bundle(
        tt,
        cell,
        feature_id,
        feature_framing,
        upstream,
        resolved_cell_path,
        audit_context,
    );

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
        allowed_tools: vec!["read_file".to_string(), "write_file".to_string()],
    };

    // FT-170: snapshot the sandbox before dispatch so placement can
    // identify exactly the files this cell's worker created.
    let before = snapshot_files(cluster_dir)?;

    let WorkerRun {
        response,
        raw_stdout: _,
    } = run_worker(argv, &payload).with_context(|| {
        format!(
            "dispatch code-writer for cell {}/{} (feature {})",
            tt.name, cell.name, feature_id
        )
    })?;

    // FT-170: the harness owns placement — the worker's chosen path is
    // advisory; the artifact ends up at the resolved path or the cell
    // fails with a diagnostic naming the expectation. Cells with an
    // empty output_path (FT-145-era flat convention) keep the FT-139
    // read-back behaviour unchanged.
    if !cell.output_path.as_os_str().is_empty() {
        let cell_label = format!("{}/{}", tt.name, cell.name);
        place_cell_output(cluster_dir, resolved_cell_path, &before, &cell_label)?;
    }

    // Read back what the worker wrote (post-placement, guaranteed path).
    let written_path = cluster_dir.join(resolved_cell_path);
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
    audit_context: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Cluster dispatch: {} / {}\n\n",
        tt.name, cell.name
    ));
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
    // FT-171: a retried cell sees exactly why the audit rejected the
    // prior attempt — the diagnostic is corrective context, not noise.
    if let Some(audit) = audit_context {
        out.push_str("## Prior audit failure\n\n");
        out.push_str(
            "Your previous output for this cell failed the cluster's coherence audit. \
             Fix the issue the audit names below — change only what the diagnostic requires.\n\n```\n",
        );
        out.push_str(audit);
        out.push_str("\n```\n\n");
    }
    out.push_str("## Your task\n\n");
    out.push_str(&format!(
        "Emit the `{}` cell of the `{}` task type. The artifact type is `{}`.\n\n",
        cell.name, tt.name, cell.artifact_type
    ));
    // FT-165: explicit write_file invariant. The previous instruction
    // ("Write a single file ... When you have written the file, end your
    // turn") admitted a "I'll paste the content in my response and end my
    // turn" failure mode that aborted ~50% of FT-147 substrate cells.
    // The numbered workflow + named tool + forbidden-modes list removes
    // the ambiguity without changing the cluster's structural contract.
    out.push_str("### Required workflow\n\n");
    out.push_str(&format!(
        "1. Call the `write_file` tool with:\n   \
           - `path`: `{}`\n   \
           - `content`: the **complete** file body — no placeholders, no TODO markers, no \"rest of file unchanged\".\n",
        target_filename.display()
    ));
    out.push_str(
        "2. The dispatch is INCOMPLETE until your `write_file` tool call returns success.\n\
         3. Do not paste the file content into your assistant message text — that is NOT writing the file. Only a `write_file` tool call counts.\n\
         4. Do not create any other files. The target path is the ONLY file you may write.\n\
         5. After `write_file` returns success, respond with a single line confirming success and end your turn.\n\n",
    );
    out.push_str("### Failure modes to avoid\n\n");
    out.push_str(
        "- Responding with file content in markdown but never calling `write_file` → the dispatch reads zero bytes and aborts.\n\
         - Calling `write_file` with partial content and a placeholder (\"// ... rest unchanged\") → the file fails the audit downstream.\n\
         - Creating helper / scratch files alongside the target → audit rejects.\n\
         - Narrating the plan before acting — call the tool first, narrate after.\n",
    );
    out
}

/// Read the feature spec's body (truncated) to use as context framing
/// in every cell's bundle.
fn load_feature_framing(product_root: &Path, feature_id: &str) -> Result<String> {
    let glob_pattern = format!(".product/features/{}-*.md", feature_id);
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

/// FT-171: structured audit verdict so the repair loop can map FAIL
/// lines back to cells. Unrunnable audits stay hard errors.
enum AuditOutcome {
    Pass,
    Fail {
        fail_lines: Vec<String>,
        raw: String,
    },
}

/// FT-171: direct dependents of `cell_name` in the TaskType's
/// `derived_from` graph.
fn dependents_of(tt: &TaskTypeDecl, cell_name: &str) -> Vec<String> {
    tt.cells
        .iter()
        .filter(|c| c.derived_from.iter().any(|d| d == cell_name))
        .map(|c| c.name.clone())
        .collect()
}

/// FT-171: map the audit's `FAIL check=<name>: <detail>` lines to the
/// cells to re-dispatch. Two signals, in order of precision:
///
/// 1. Path evidence — a detail mentioning a cell's resolved output_path
///    implicates that cell (canonical_namespace and compile_probe carry
///    file:line diagnostics).
/// 2. Check-name evidence — a check named after a cell (or prefixed by
///    one, e.g. `shacl_field_coverage` → `shacl_shape`) implicates it.
///
/// A FAIL line carrying neither implicates every cell — degrading to
/// today's everything-again semantics rather than silently narrowing.
fn implicate_cells(
    tt: &TaskTypeDecl,
    params: &BTreeMap<String, String>,
    fail_lines: &[String],
) -> std::collections::BTreeSet<String> {
    let mut implicated = std::collections::BTreeSet::new();
    let resolved: Vec<(String, String)> = tt
        .cells
        .iter()
        .filter_map(|c| {
            resolve_cell_output_path(c, params)
                .ok()
                .map(|p| (c.name.clone(), p.to_string_lossy().into_owned()))
        })
        .collect();

    for line in fail_lines {
        let mut matched = false;
        for (cell, path) in &resolved {
            if !path.is_empty() && line.contains(path.as_str()) {
                implicated.insert(cell.clone());
                matched = true;
            }
        }
        if !matched {
            if let Some(check) = line
                .split("check=")
                .nth(1)
                .and_then(|rest| rest.split([':', ' ']).next())
            {
                for c in &tt.cells {
                    if check == c.name || check.starts_with(&format!("{}_", prefix_of(&c.name))) {
                        implicated.insert(c.name.clone());
                        matched = true;
                    }
                }
            }
        }
        if !matched {
            // Unmapped failure — conservative: everything re-runs.
            implicated.extend(tt.cells.iter().map(|c| c.name.clone()));
        }
    }
    implicated
}

/// First underscore-delimited token of a cell name (`shacl_shape` →
/// `shacl`), used to match family checks like `shacl_field_coverage`.
fn prefix_of(cell_name: &str) -> &str {
    cell_name.split('_').next().unwrap_or(cell_name)
}

/// FT-171: audit → repair → re-audit loop. The sandbox is preserved
/// across rounds; only implicated cells re-dispatch, with the audit
/// diagnostic appended to their bundles as corrective context.
#[allow(clippy::too_many_arguments)]
fn audit_with_repair(
    ctx: &PlanContext,
    argv: &[String],
    store: &Store,
    tt: &TaskTypeDecl,
    feature_id: &str,
    task_type_name: &str,
    order: &[String],
    cluster_dir: &Path,
    feature_framing: &str,
    params: &BTreeMap<String, String>,
    cell_sessions: &mut Vec<CellSessionRecord>,
    cell_outputs: &mut BTreeMap<String, String>,
) -> Result<()> {
    let mut retry_counts: BTreeMap<String, u32> = BTreeMap::new();
    for round in 0..=MAX_AUDIT_REPAIR_ROUNDS {
        let outcome = run_coherence_audit(
            tt,
            &ctx.workdir,
            cluster_dir,
            feature_id,
            task_type_name,
            params,
        )?;
        let (fail_lines, raw) = match outcome {
            AuditOutcome::Pass => return Ok(()),
            AuditOutcome::Fail { fail_lines, raw } => (fail_lines, raw),
        };
        let audit_err = || {
            anyhow!(
                "cluster_dispatch::run: audit failed for TaskType {:?} on {}: {}",
                task_type_name,
                feature_id,
                raw
            )
        };
        if round == MAX_AUDIT_REPAIR_ROUNDS || fail_lines.is_empty() {
            return Err(audit_err());
        }
        let implicated = implicate_cells(tt, params, &fail_lines);
        if implicated.is_empty() {
            return Err(audit_err());
        }
        for cell in &implicated {
            let count = retry_counts.entry(cell.clone()).or_insert(0);
            if *count >= MAX_CELL_RETRIES {
                return Err(anyhow!(
                    "cluster_dispatch::run: audit failed for TaskType {:?} on {} and cell {}                      exhausted its {} retries: {}",
                    task_type_name,
                    feature_id,
                    cell,
                    MAX_CELL_RETRIES,
                    raw
                ));
            }
            *count += 1;
        }
        tracing::info!(
            feature_id,
            task_type = task_type_name,
            round,
            cells = ?implicated,
            "cluster audit failed; re-dispatching implicated cells with the diagnostic (FT-171)"
        );
        let audit_context = fail_lines.join("\n");
        run_cells(
            ctx,
            argv,
            store,
            tt,
            feature_id,
            task_type_name,
            order,
            cluster_dir,
            feature_framing,
            params,
            cell_sessions,
            cell_outputs,
            Some(&implicated),
            Some(&audit_context),
        )?;
    }
    unreachable!("loop returns on pass, cap, or unmapped failure");
}

fn run_coherence_audit(
    tt: &TaskTypeDecl,
    workdir: &Path,
    cluster_dir: &Path,
    feature_id: &str,
    task_type_name: &str,
    params: &BTreeMap<String, String>,
) -> Result<AuditOutcome> {
    let audit = &tt.coherence_audit;
    let audit_path = workdir.join(&audit.script_path);
    if !audit_path.exists() {
        tracing::warn!(
            feature_id,
            task_type = task_type_name,
            script = %audit.script_path.display(),
            "cluster_dispatch: audit script not present; treating as deferred"
        );
        return Ok(AuditOutcome::Pass);
    }
    // FT-172: pass each cell's resolved output path (relative to the
    // fixture) after the fixture dir so content checks (compile probe,
    // namespace) audit exactly the declared cell set and ignore any
    // worker-fabricated extras. Pre-FT-172 audits ignore extra argv.
    let mut cell_paths = Vec::new();
    for cell in &tt.cells {
        if let Ok(p) = resolve_cell_output_path(cell, params) {
            cell_paths.push(p);
        }
    }
    let output = Command::new(&audit_path)
        .current_dir(workdir)
        // Pass the cluster sandbox dir explicitly — audits expect a
        // fixture path as $1.
        .arg(cluster_dir)
        .args(&cell_paths)
        .output()
        .with_context(|| format!("invoke coherence audit at {}", audit_path.display()))?;
    match output.status.code() {
        Some(0) => Ok(AuditOutcome::Pass),
        Some(1) => {
            let raw = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let fail_lines = raw
                .lines()
                .filter(|l| l.contains("FAIL check="))
                .map(str::to_string)
                .collect();
            Ok(AuditOutcome::Fail { fail_lines, raw })
        }
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

    fn add_artifact_type_tt() -> &'static TaskTypeDecl {
        task_type::lookup("add-artifact-type").expect("registry has add-artifact-type")
    }

    fn archetype_params() -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        p.insert("artifact_name".to_string(), "archetype".to_string());
        p
    }

    /// FT-171: a FAIL line carrying a cell's resolved output_path
    /// implicates exactly that cell (path evidence beats check names).
    #[test]
    fn ft_171_implicate_by_path_evidence() {
        let tt = add_artifact_type_tt();
        let lines = vec![
            "FAIL check=canonical_namespace: non-canonical IRI base(s): \
             crates/dec-ontology/src/vocab/archetype.rs:3: https://bad.example/x"
                .to_string(),
        ];
        let got = implicate_cells(tt, &archetype_params(), &lines);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            got.contains("iri_module_consts") || got.contains("iri_constants"),
            "{got:?}"
        );
    }

    /// FT-171: a FAIL whose check name matches a cell (or its prefix
    /// family) implicates that cell.
    #[test]
    fn ft_171_implicate_by_check_name() {
        let tt = add_artifact_type_tt();
        let lines = vec!["FAIL check=shacl_field_coverage: missing sh:path".to_string()];
        let got = implicate_cells(tt, &archetype_params(), &lines);
        assert!(got.contains("shacl_shape"), "{got:?}");
        assert!(
            got.len() < tt.cells.len(),
            "must not degrade to all cells: {got:?}"
        );
    }

    /// FT-171: an unmapped FAIL implicates every cell — degrading to
    /// today's everything-again semantics, never silently narrower.
    #[test]
    fn ft_171_unmapped_failure_implicates_all_cells() {
        let tt = add_artifact_type_tt();
        let lines = vec!["FAIL check=mystery: something nobody owns".to_string()];
        let got = implicate_cells(tt, &archetype_params(), &lines);
        assert_eq!(got.len(), tt.cells.len(), "{got:?}");
    }

    /// FT-171: a retried cell's prompt carries the audit diagnostic as
    /// a dedicated section.
    #[test]
    fn ft_171_bundle_carries_prior_audit_failure() {
        let tt = add_artifact_type_tt();
        let cell = &tt.cells[0];
        let upstream = BTreeMap::new();
        let bundle = build_cell_bundle(
            tt,
            cell,
            "FT-T171",
            "framing",
            &upstream,
            &PathBuf::from("x.rs"),
            Some("FAIL check=compile_probe: expected `;`"),
        );
        assert!(bundle.contains("## Prior audit failure"), "{bundle}");
        assert!(bundle.contains("FAIL check=compile_probe"), "{bundle}");
    }

    /// FT-171: dependents_of walks the derived_from graph one level.
    #[test]
    fn ft_171_dependents_of_direct_edges() {
        let tt = add_artifact_type_tt();
        // rust_struct feeds shacl_shape/parser/emitter in this cluster.
        let deps = dependents_of(tt, "rust_struct");
        assert!(!deps.is_empty(), "{deps:?}");
        for d in &deps {
            let cell = tt.cells.iter().find(|c| &c.name == d).unwrap();
            assert!(cell.derived_from.iter().any(|x| x == "rust_struct"));
        }
    }

    /// FT-170 case 1: the worker honoured the resolved path — no-op,
    /// file content untouched, stray extras tolerated.
    #[test]
    fn ft_170_placement_noop_when_resolved_path_written() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let before = snapshot_files(tmp.path()).unwrap();
        let resolved = PathBuf::from("crates/x/src/thing.rs");
        fs::create_dir_all(tmp.path().join("crates/x/src")).unwrap();
        fs::write(tmp.path().join(&resolved), "pub struct T;").unwrap();
        fs::write(tmp.path().join("crates/x/src/extra.rs"), "// helper").unwrap();

        place_cell_output(tmp.path(), &resolved, &before, "tt/cell").expect("case 1 is a no-op");
        let body = fs::read_to_string(tmp.path().join(&resolved)).unwrap();
        assert_eq!(body, "pub struct T;");
    }

    /// FT-170 case 2: one stray of the right kind — relocated to the
    /// resolved path with content preserved; the stray is gone.
    #[test]
    fn ft_170_placement_relocates_single_stray() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let before = snapshot_files(tmp.path()).unwrap();
        let resolved = PathBuf::from("crates/x/src/shapes/thing.shacl.ttl");
        // Worker drifted: wrote under a nested dir of its own invention.
        let stray = PathBuf::from("crates/x/src/thing/shapes/thing.shacl.ttl");
        fs::create_dir_all(tmp.path().join(stray.parent().unwrap())).unwrap();
        fs::write(tmp.path().join(&stray), "@prefix dec: <x> .").unwrap();

        place_cell_output(tmp.path(), &resolved, &before, "tt/cell").expect("case 2 relocates");
        let body = fs::read_to_string(tmp.path().join(&resolved)).unwrap();
        assert_eq!(body, "@prefix dec: <x> .");
        assert!(
            !tmp.path().join(&stray).exists(),
            "stray must be moved, not copied"
        );
    }

    /// FT-170 case 3: nothing of the right kind — the cell fails with a
    /// diagnostic naming the expected path.
    #[test]
    fn ft_170_placement_fails_when_nothing_of_right_kind() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let before = snapshot_files(tmp.path()).unwrap();
        let resolved = PathBuf::from("crates/x/src/thing.rs");
        // Worker wrote only a different kind of file.
        fs::write(tmp.path().join("notes.md"), "thoughts").unwrap();

        let err = place_cell_output(tmp.path(), &resolved, &before, "tt/cell")
            .expect_err("case 3 must fail");
        assert!(err.to_string().contains("crates/x/src/thing.rs"), "{err}");
    }

    /// FT-170 case 4: several candidates and none at the resolved path —
    /// ambiguity is never silently resolved; the diagnostic lists them.
    #[test]
    fn ft_170_placement_fails_on_ambiguous_candidates() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let before = snapshot_files(tmp.path()).unwrap();
        let resolved = PathBuf::from("thing.rs");
        fs::write(tmp.path().join("a.rs"), "a").unwrap();
        fs::write(tmp.path().join("b.rs"), "b").unwrap();

        let err = place_cell_output(tmp.path(), &resolved, &before, "tt/cell")
            .expect_err("case 4 must fail");
        let msg = err.to_string();
        assert!(msg.contains("a.rs") && msg.contains("b.rs"), "{msg}");
    }

    /// FT-170 invariant (amended by the witnessed FT-148 run): the
    /// resolved path is this cell's own slot, so stale content there —
    /// a prior attempt's partial output — is replaced by the fresh
    /// stray, never protected.
    #[test]
    fn ft_170_placement_replaces_stale_own_output() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let resolved = PathBuf::from("thing.rs");
        fs::write(tmp.path().join(&resolved), "stale prior attempt").unwrap();
        let before = snapshot_files(tmp.path()).unwrap();
        // The retried cell drifts while its slot still holds old content.
        fs::write(tmp.path().join("stray.rs"), "fresh content").unwrap();

        place_cell_output(tmp.path(), &resolved, &before, "tt/cell")
            .expect("stale own output is replaced");
        let body = fs::read_to_string(tmp.path().join(&resolved)).unwrap();
        assert_eq!(body, "fresh content");
        assert!(!tmp.path().join("stray.rs").exists());
    }

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
        std::fs::write(
            tmp.path().join(".dec/task-types.toml"),
            "this is not [valid toml",
        )
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

    fn fixture_bundle() -> String {
        let tt = TaskTypeDecl {
            name: "add-artifact-type".to_string(),
            cells: vec![],
            coherence_audit: crate::core::task_type::CoherenceAuditSpec {
                script_path: PathBuf::from("scripts/checks/x.py"),
                timeout_seconds: 60,
            },
            parameters: vec![],
        };
        let cell = CellDecl {
            name: "emitter".to_string(),
            artifact_type: "rust-source".to_string(),
            prompt_template_path: PathBuf::from("/tmp/x"),
            model_binding_capability_id: "implementer".to_string(),
            derived_from: vec![],
            output_path: PathBuf::new(),
        };
        let upstream = BTreeMap::new();
        build_cell_bundle(
            &tt,
            &cell,
            "FT-T403",
            "## Description\nFixture.\n",
            &upstream,
            &PathBuf::from("emitter.rs"),
            None,
        )
    }

    /// FT-165 TC: bundle names `write_file` and routes through the
    /// numbered "Required workflow" — pins the explicit-tool-call
    /// instruction shape. A worker reading this bundle has no
    /// ambiguity about what to call.
    #[test]
    fn ft_165_bundle_requires_write_file_tool_call_explicitly() {
        let bundle = fixture_bundle();
        assert!(
            bundle.contains("### Required workflow"),
            "bundle must surface the Required workflow heading: {bundle}"
        );
        assert!(
            bundle.contains("Call the `write_file` tool with:"),
            "bundle must name write_file explicitly: {bundle}"
        );
        assert!(
            bundle.contains("`path`: `emitter.rs`"),
            "bundle must pin the path argument: {bundle}"
        );
    }

    /// FT-165 TC: bundle forbids text-only responses. Pins the
    /// anti-narrate guard so a worker pasting content into its assistant
    /// message can't claim that satisfies the dispatch.
    #[test]
    fn ft_165_bundle_forbids_pasting_content_in_text() {
        let bundle = fixture_bundle();
        assert!(
            bundle.contains("Do not paste the file content into your assistant message text"),
            "anti-paste guard missing: {bundle}"
        );
        assert!(
            bundle.contains("Only a `write_file` tool call counts"),
            "explicit \"only tool call counts\" rule missing: {bundle}"
        );
    }

    /// FT-165 TC: bundle caps file creation at the single target.
    /// Removes the witnessed "let me also create a helper" failure
    /// mode (stray `product.verify` file on FT-147 retry).
    #[test]
    fn ft_165_bundle_caps_writes_at_single_target() {
        let bundle = fixture_bundle();
        assert!(
            bundle.contains("Do not create any other files."),
            "single-target rule missing: {bundle}"
        );
        assert!(
            bundle.contains("The target path is the ONLY file you may write."),
            "ONLY-target emphasis missing: {bundle}"
        );
    }

    /// FT-165 TC: bundle keeps the dispatch-incomplete-until-tool-success
    /// invariant + the failure-modes list. These framings together force
    /// the worker to internalize "tool call = success, anything else =
    /// failure".
    #[test]
    fn ft_165_bundle_emphasises_dispatch_incomplete_until_tool_call() {
        let bundle = fixture_bundle();
        assert!(
            bundle.contains(
                "dispatch is INCOMPLETE until your `write_file` tool call returns success"
            ),
            "INCOMPLETE-until-tool-success invariant missing: {bundle}"
        );
        assert!(
            bundle.contains("### Failure modes to avoid"),
            "failure-modes section missing: {bundle}"
        );
        assert!(
            bundle.contains("never calling `write_file`"),
            "explicit never-calling-write_file mention missing: {bundle}"
        );
    }

    /// FT-166 TC: substitute_params replaces every `{name}` placeholder
    /// against the resolved map. Literal substitution (no escaping, no
    /// nesting); leaves unmatched placeholders untouched.
    #[test]
    fn ft_166_substitute_params_replaces_placeholders() {
        let mut params = BTreeMap::new();
        params.insert("artifact_name".to_string(), "archetype".to_string());
        params.insert("crate_path".to_string(), "dec-ontology".to_string());
        let template = "crates/{crate_path}/src/ontology/{artifact_name}/parser.rs";
        let resolved = substitute_params(template, &params);
        assert_eq!(
            resolved,
            "crates/dec-ontology/src/ontology/archetype/parser.rs"
        );
        // Unmatched placeholder stays literal — no panic, no silent drop.
        let untouched = substitute_params("{unknown}/x", &params);
        assert_eq!(untouched, "{unknown}/x");
    }

    /// FT-166 TC: resolve_cell_output_path returns the substituted path
    /// when output_path is set, falls back to flat-path convention when
    /// empty (backwards-compat with FT-145's add-cli-subcommand cluster).
    #[test]
    fn ft_166_resolve_cell_output_path_substitutes_or_falls_back() {
        let mut params = BTreeMap::new();
        params.insert("artifact_name".to_string(), "feedback".to_string());

        // Templated path — substituted.
        let cell_templated = CellDecl {
            name: "rust_struct".to_string(),
            artifact_type: "rust-source".to_string(),
            prompt_template_path: PathBuf::new(),
            model_binding_capability_id: "implementer".to_string(),
            derived_from: vec![],
            output_path: PathBuf::from("crates/dec-ontology/src/ontology/{artifact_name}.rs"),
        };
        let resolved = resolve_cell_output_path(&cell_templated, &params).unwrap();
        assert_eq!(
            resolved,
            PathBuf::from("crates/dec-ontology/src/ontology/feedback.rs")
        );

        // Empty output_path — falls back to flat convention (cell_filename).
        let cell_flat = CellDecl {
            name: "clap_args_module".to_string(),
            artifact_type: "rust-source".to_string(),
            prompt_template_path: PathBuf::new(),
            model_binding_capability_id: "implementer".to_string(),
            derived_from: vec![],
            output_path: PathBuf::new(),
        };
        let resolved_flat = resolve_cell_output_path(&cell_flat, &params).unwrap();
        assert_eq!(resolved_flat, PathBuf::from("clap_args_module.rs"));
    }

    /// FT-166 TC: required parameter (no default + no override) surfaces
    /// a clean error before any cell dispatch. Operator gets the
    /// missing-parameter diagnostic naming the TOML location to populate.
    #[test]
    fn ft_166_resolve_parameters_fails_fast_on_missing_required_param() {
        use crate::core::task_type::TaskTypeParameter;
        let tt = TaskTypeDecl {
            name: "test-required-param".to_string(),
            cells: vec![],
            coherence_audit: crate::core::task_type::CoherenceAuditSpec {
                script_path: PathBuf::from("x"),
                timeout_seconds: 60,
            },
            parameters: vec![TaskTypeParameter {
                name: "artifact_name".to_string(),
                description: "Test required parameter.".to_string(),
                default: None,
            }],
        };
        // Tempdir has no .dec/task-types.toml → param absent.
        let tmp = tempfile::tempdir().unwrap();
        let err =
            resolve_parameters(&tt, tmp.path(), "FT-Tmissing").expect_err("required param missing");
        let msg = format!("{err}");
        assert!(
            msg.contains("requires parameter `artifact_name`"),
            "missing-parameter diagnostic does not name the parameter: {msg}"
        );
        assert!(
            msg.contains("[parameters.\"FT-Tmissing\"]"),
            "diagnostic does not surface the TOML table to populate: {msg}"
        );
    }

    /// FT-166 TC: path-traversal guard rejects resolved paths containing
    /// `..` segments. Sandbox containment is structural; a malicious or
    /// misconfigured parameter cannot escape the cluster sandbox.
    #[test]
    fn ft_166_resolve_cell_output_path_rejects_dotdot_traversal() {
        let mut params = BTreeMap::new();
        // Operator typo or hostile config injects "..".
        params.insert("artifact_name".to_string(), "../../etc/passwd".to_string());
        let cell = CellDecl {
            name: "rust_struct".to_string(),
            artifact_type: "rust-source".to_string(),
            prompt_template_path: PathBuf::new(),
            model_binding_capability_id: "implementer".to_string(),
            derived_from: vec![],
            output_path: PathBuf::from("crates/{artifact_name}.rs"),
        };
        let err = resolve_cell_output_path(&cell, &params).expect_err("traversal must reject");
        let msg = format!("{err}");
        assert!(
            msg.contains("sandbox containment guard"),
            "traversal diagnostic missing: {msg}"
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
            .find(|e| e.file_name().to_string_lossy().starts_with(prefix));
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
