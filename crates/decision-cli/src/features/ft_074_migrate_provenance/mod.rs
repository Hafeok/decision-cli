//! `dec migrate provenance` — migration tooling for dual-provenance
//! conformance (FT-074 / ADR-042).
//!
//! Slice-1 surface: a one-shot migration that walks every existing
//! Feature / ADR / TC / Dependency artifact, classifies it via the
//! three-class audit ([`audit`]), backfills synthetic mechanical
//! provenance for backfillable cases ([`backfill`]), flags orphans via
//! Feedback ([`orphan_feedback`]), and lands a `:warnOnlyMode` flag on
//! the orchestration store until the operator runs cutover.
//!
//! The migration tool itself conforms to the dual-provenance discipline
//! — synthetic Sessions and Agents carry `BoundaryArtifact /
//! MigrationBackfill` class membership plus `:external_origin`, so
//! recursion terminates at the boundary (FT-074 §Invariants).
//!
//! Per-feature boundary: this module is the only entry point for the
//! migration logic; the CLI adapter in `cli::migrate` delegates here.

#![allow(missing_docs)]

pub mod audit;
pub mod backfill;
pub mod commands;
pub mod cutover;
pub mod mapping;
pub mod orphan_feedback;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};

pub use audit::{audit_store, AuditEntry, AuditVerdict, EdgeMap};
pub use backfill::{
    emit_backfill_quads, emit_shared_agent_quads, historical_session_iri, plan_backfill,
    BackfillPlan, HISTORICAL_AGENT_CLASS, HISTORICAL_AGENT_IRI, HISTORICAL_SESSION_CLASS,
    IRI_DEC_MIGRATION_NOTE,
};
pub use cutover::{
    count_unrepaired_orphans, run_cutover, set_warn_only_mode, warn_only_mode, CutoverOutcome,
    IRI_DEC_VALIDATOR_CONFIG_SUBJECT, IRI_DEC_WARN_ONLY_MODE,
};
pub use orphan_feedback::{
    artifact_already_marked_orphan, emit_orphan_feedback_quads, feedback_already_exists_for_orphan,
    orphan_feedback_iri, plan_orphan_feedback, OrphanFeedbackPlan, IRI_DEC_IS_MIGRATION_ORPHAN,
    MIGRATION_ORPHAN_FEEDBACK_CLASS, ORPHAN_TARGET_ROLE,
};

/// Configuration knobs for the migration run.
#[derive(Debug, Clone)]
pub struct MigrateArgs {
    /// Stable identifier for this migration run. Used to derive
    /// deterministic IRIs for synthetic Sessions and Feedback artifacts
    /// so re-running the migration is idempotent (FT-074 §Behaviour step 7).
    pub run_id: String,
    /// `xsd:dateTime` literal stamped on synthetic mechanical-provenance
    /// triples when the per-artifact git lookup is unavailable.
    pub fallback_timestamp: String,
    /// External-origin literal stamped on synthetic Session + Agent
    /// artifacts to satisfy the `:BoundaryArtifactShape` requirement.
    /// Typically `"FT-074 provenance migration tool run at <timestamp>"`.
    pub external_origin: String,
    /// Cutover threshold consulted by the cutover sub-command. Defaults
    /// to zero (operator-overridable; FT-074 §Behaviour step 6).
    pub cutover_threshold: usize,
    /// `true` for `--dry-run`; the audit pass runs but no mutations are
    /// committed and no report file is written.
    pub dry_run: bool,
}

impl Default for MigrateArgs {
    fn default() -> Self {
        Self {
            run_id: "default".to_string(),
            fallback_timestamp: "2026-05-25T00:00:00Z".to_string(),
            external_origin: "FT-074 provenance migration tool — fallback run identity".to_string(),
            cutover_threshold: 0,
            dry_run: false,
        }
    }
}

/// One row in the migration report — pairs an audit entry with the
/// outcome of applying it (or `Planned` in dry-run mode).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRow {
    /// Artifact IRI under audit.
    pub artifact: String,
    /// `rdf:type` IRI of the artifact.
    pub rdf_type: String,
    /// Audit verdict for the artifact.
    pub verdict: AuditVerdict,
    /// What the migration tool did with this row.
    pub outcome: RowOutcome,
}

/// Per-row outcome — captures backfill / orphan emission / skip state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RowOutcome {
    /// Conformant artifact — no work needed.
    SkippedConformant,
    /// Backfillable artifact — planned but not committed (dry-run).
    PlannedBackfill {
        session_iri: String,
        generated_at_time: String,
    },
    /// Backfillable artifact — committed.
    AppliedBackfill {
        session_iri: String,
        generated_at_time: String,
    },
    /// Orphan artifact — planned (dry-run) or committed.
    PlannedOrphanFeedback { feedback_iri: String },
    /// Orphan artifact — committed.
    AppliedOrphanFeedback { feedback_iri: String },
    /// Orphan feedback skipped because it already exists in the store
    /// (cross-run idempotence).
    SkippedAlreadyOrphanFlagged { feedback_iri: String },
    /// Backfill skipped because mechanical block already present and
    /// motivational already maps (FT-074 §Behaviour step 7).
    SkippedAlreadyBackfilled,
}

/// Aggregate report — written to `.product/.migrations/provenance-<run_id>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub run_id: String,
    pub generated_at: String,
    pub dry_run: bool,
    pub rows: Vec<ReportRow>,
    pub summary: ReportSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total: usize,
    pub conformant: usize,
    pub backfilled: usize,
    pub orphan: usize,
    pub already_orphan: usize,
    pub already_backfilled: usize,
}

/// Run the migration end-to-end against the in-memory store. Pure: does
/// not write anything outside the store. Caller is responsible for
/// persisting the store and writing the report file.
pub fn run_migration(store: &Store, args: &MigrateArgs) -> Result<MigrationReport> {
    let entries = audit_store(store)?;
    let mut rows = Vec::with_capacity(entries.len());
    let mut summary = ReportSummary::default();
    let mut agent_emitted_this_run = false;

    for entry in &entries {
        summary.total += 1;
        let outcome = match &entry.verdict {
            AuditVerdict::Conformant => {
                summary.conformant += 1;
                RowOutcome::SkippedConformant
            }
            AuditVerdict::BackfillableMechanical { .. } => process_backfill(
                store,
                args,
                entry,
                &mut summary,
                &mut agent_emitted_this_run,
            )?,
            AuditVerdict::Orphan { reasons } => {
                process_orphan(store, args, entry, reasons, &mut summary)?
            }
        };
        rows.push(ReportRow {
            artifact: entry.artifact.clone(),
            rdf_type: entry.rdf_type.clone(),
            verdict: entry.verdict.clone(),
            outcome,
        });
    }

    Ok(MigrationReport {
        run_id: args.run_id.clone(),
        generated_at: args.fallback_timestamp.clone(),
        dry_run: args.dry_run,
        rows,
        summary,
    })
}

fn process_backfill(
    store: &Store,
    args: &MigrateArgs,
    entry: &AuditEntry,
    summary: &mut ReportSummary,
    agent_emitted_this_run: &mut bool,
) -> Result<RowOutcome> {
    let artifact = NamedNode::new(&entry.artifact).map_err(|e| anyhow!("bad artifact IRI: {e}"))?;
    let plan = plan_backfill(
        &artifact,
        &args.run_id,
        &args.fallback_timestamp,
        &format!("Pre-discipline {} migrated by FT-074", entry.rdf_type),
    );

    if args.dry_run {
        summary.backfilled += 1;
        return Ok(RowOutcome::PlannedBackfill {
            session_iri: plan.session.as_str().to_string(),
            generated_at_time: plan.generated_at_time.clone(),
        });
    }

    // Apply path — write through oxi-events GraphWriter directly so we
    // bypass the StreamWriter dec:inStream augmentation (the migration
    // is not stream-scoped). Use the underlying Store's transaction
    // API directly so this module remains importable from tests that
    // don't bootstrap a full ValueStream.
    let mut quads = emit_backfill_quads(&plan, &args.external_origin);
    if !*agent_emitted_this_run {
        quads.extend(emit_shared_agent_quads(&args.external_origin));
        *agent_emitted_this_run = true;
    }
    commit_quads(store, &quads)?;
    summary.backfilled += 1;
    Ok(RowOutcome::AppliedBackfill {
        session_iri: plan.session.as_str().to_string(),
        generated_at_time: plan.generated_at_time.clone(),
    })
}

fn process_orphan(
    store: &Store,
    args: &MigrateArgs,
    entry: &AuditEntry,
    reasons: &[String],
    summary: &mut ReportSummary,
) -> Result<RowOutcome> {
    let artifact = NamedNode::new(&entry.artifact).map_err(|e| anyhow!("bad artifact IRI: {e}"))?;
    let plan = plan_orphan_feedback(&artifact, &entry.rdf_type, reasons, &args.run_id);

    if args.dry_run {
        summary.orphan += 1;
        return Ok(RowOutcome::PlannedOrphanFeedback {
            feedback_iri: plan.feedback_iri.as_str().to_string(),
        });
    }

    if feedback_already_exists_for_orphan(store, entry.artifact.as_str())? {
        summary.already_orphan += 1;
        return Ok(RowOutcome::SkippedAlreadyOrphanFlagged {
            feedback_iri: plan.feedback_iri.as_str().to_string(),
        });
    }

    let quads = emit_orphan_feedback_quads(&plan);
    commit_quads(store, &quads)?;
    summary.orphan += 1;
    Ok(RowOutcome::AppliedOrphanFeedback {
        feedback_iri: plan.feedback_iri.as_str().to_string(),
    })
}

fn commit_quads(store: &Store, quads: &[oxigraph::model::Quad]) -> Result<()> {
    // Avoid double-inserting on re-runs: skip quads already present.
    store
        .transaction(|mut tx| {
            for q in quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<(), oxigraph::store::StorageError>(())
        })
        .map_err(|e| anyhow!("migration commit failed: {e}"))
}

/// Persist the migration report to disk at the canonical path.
pub fn write_report(workdir: &Path, report: &MigrationReport) -> Result<PathBuf> {
    let dir = workdir.join(".product").join(".migrations");
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(format!("provenance-{}.json", report.run_id));
    let body = serde_json::to_string_pretty(report).context("serialising report")?;
    fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// Bypass `unwrap_used` lint — `_import_anchor` keeps the `Mutation`
// import live for the slice-2 StreamWriter-aware commit path.
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub fn _import_anchor() -> Mutation {
    Mutation::insert(Vec::<oxigraph::model::Quad>::new())
}
