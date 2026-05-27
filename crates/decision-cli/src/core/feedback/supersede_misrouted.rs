//! FT-110 follow-up: supersede pre-TC-199 feedback that was misrouted
//! to the `implementer` because the verdict aggregator hadn't yet
//! learned to demote graph-fault exit codes.
//!
//! Walks every open `class=defect targetRole=implementer` feedback in
//! the store, matches its evidence excerpt against the graph-fault
//! exit-code signature (`expected exit 0, got {2|126|127}`), mints a
//! corrected twin (`targetRole = "verifier"`, same source artifact +
//! evidence + session), and transitions the original to `superseded`
//! with `dec:supersededBy` pointing at the twin.
//!
//! After this runs, the `dec drive ship FT-XXX` planner will route
//! the corrected feedbacks to the verify-graph-author, which is the
//! right place to fix graph-design issues like `dec init` being
//! invoked without `--template` / `--from`.

use std::sync::Arc;

use anyhow::{Context, Result};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use uuid::Uuid;

use crate::core::feedback::artifact::{Feedback, Severity};
use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::feedback::read::list_by_class;
use crate::core::feedback::transition::apply as apply_transition;
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::orchestration_graph;

/// Summary of one supersession sweep.
#[derive(Debug, Default, Clone)]
pub struct SupersedeReport {
    /// Total candidate feedbacks scanned.
    pub scanned: usize,
    /// Candidates whose evidence matched a graph-fault exit code.
    pub matched: usize,
    /// Feedbacks successfully superseded.
    pub superseded: usize,
    /// Errors encountered (logged but not fatal — partial progress
    /// commits before bail-out).
    pub errors: Vec<String>,
}

/// Run the sweep over the orchestration store at `workdir`.
///
/// Idempotent: feedback already in a terminal state (addressed /
/// closed / rejected / superseded) is skipped automatically. Re-running
/// only affects net-new graph-fault implementer feedback that's
/// landed since the previous sweep.
///
/// `dry_run` mode reports what would have been superseded without
/// committing any quads — useful for previewing the blast radius
/// before a real run.
pub fn supersede_misrouted_implementer_defects(
    workdir: &std::path::Path,
    dry_run: bool,
) -> Result<SupersedeReport> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)
        .with_context(|| format!("loading orchestration store at {}", dump.display()))?;
    let store = Arc::new(store);
    let scope =
        ActiveScope::load(workdir).context("loading active scope for supersession")?;
    let stream_iri =
        NamedNode::new(&scope.stream_iri).context("active stream iri")?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri.clone())
        .context("opening writer for supersession")?;

    let mut report = SupersedeReport::default();
    let defects = list_by_class(&store, "defect")
        .map_err(|e| anyhow::anyhow!("listing defect feedback: {e}"))?;

    for fb in defects {
        report.scanned += 1;
        if fb.target_role != "implementer" {
            continue;
        }
        if !matches!(
            fb.lifecycle_state.as_str(),
            "produced" | "routed" | "received"
        ) {
            continue;
        }
        if !evidence_signals_graph_fault(&fb.evidence) {
            continue;
        }
        report.matched += 1;
        if dry_run {
            continue;
        }
        match supersede_one(&store, &writer, &fb, &stream_iri) {
            Ok(()) => report.superseded += 1,
            Err(e) => report.errors.push(format!("{}: {e:#}", fb.iri.as_str())),
        }
    }

    if !dry_run && (report.superseded > 0 || !report.errors.is_empty()) {
        persist_store(&store, &dump)
            .with_context(|| format!("persisting store after supersession"))?;
    }
    Ok(report)
}

/// Returns `true` if `evidence` carries one of the graph-fault
/// signatures the per-step routing (FT-110.X) now catches at
/// emission time. Each pattern below corresponds to a step-kind
/// failure mode the new emission-time rule routes to the verifier:
///
///   * `expected exit 0, got {2|126|127}` — shell graph-fault exit
///     codes (the original supersede-script scope).
///   * `expected N rows, got M` — sparql-assertion row-count
///     mismatch. The verify-graph-author authored both the query
///     and the expected count, so a mismatch is almost always a
///     graph-design issue (ASK-vs-SELECT, missing GRAPH wrapping,
///     wrong predicate name).
///   * `file missing:` — file-assertion path mismatch. Same logic:
///     the verifier picked the path, a mismatch is graph-side.
///
/// The patterns are tail-matched (substring containment) so the
/// surrounding `"step N produced outcome fail; …"` framing is fine.
fn evidence_signals_graph_fault(evidence: &str) -> bool {
    for code in [2_i64, 126, 127] {
        let needle = format!("expected exit 0, got {code}");
        if evidence.contains(&needle) {
            return true;
        }
    }
    // sparql-assertion row-count mismatch — verifier-authored, so
    // the wrong-shape query (ASK with expect-rows, missing GRAPH
    // wrapping, wrong predicate) is the most likely cause.
    if evidence.contains(" rows, got ") {
        return true;
    }
    // file-assertion path mismatch.
    if evidence.contains("file missing:") {
        return true;
    }
    false
}

/// Escalate every open `targetRole = "verifier"` defect feedback whose
/// `dec:sourceArtifact` is one of `tc_iris` by superseding it with a
/// twin that has `targetRole = "implementer"`. Used by the FT-110
/// planner's `EscalateVgaToImplementer` path — when the
/// verify-graph-author can't make progress, the next thing to try is
/// "ask the implementer to fix what the verifier was complaining
/// about." Returns the number of feedbacks successfully escalated.
///
/// `tc_iris` should be the full IRI form
/// (`https://decision-cli.dev/ns/tc/TC-NNN`); the caller derives them
/// via `resolve_feature_tcs_short` + `tc_iri_for`.
///
/// Idempotent: feedbacks already in a terminal state are skipped.
pub fn escalate_verifier_defects_to_implementer(
    workdir: &std::path::Path,
    tc_iris: &[String],
) -> Result<usize> {
    if tc_iris.is_empty() {
        return Ok(0);
    }
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)
        .with_context(|| format!("loading orchestration store at {}", dump.display()))?;
    let store = Arc::new(store);
    let scope = ActiveScope::load(workdir).context("loading active scope")?;
    let stream_iri = NamedNode::new(&scope.stream_iri).context("active stream iri")?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri.clone())
        .context("opening writer for escalation")?;

    let tc_set: std::collections::HashSet<&str> =
        tc_iris.iter().map(String::as_str).collect();

    let defects = list_by_class(&store, "defect")
        .map_err(|e| anyhow::anyhow!("listing defect feedback: {e}"))?;
    let mut escalated = 0_usize;
    for fb in defects {
        if fb.target_role != "verifier" {
            continue;
        }
        if !matches!(
            fb.lifecycle_state.as_str(),
            "produced" | "routed" | "received"
        ) {
            continue;
        }
        let Some(source) = fb.source_artifact.as_ref() else {
            continue;
        };
        if !tc_set.contains(source.as_str()) {
            continue;
        }
        if supersede_with_role_twin(&store, &writer, &fb, &stream_iri, "implementer").is_ok() {
            escalated += 1;
        }
    }
    if escalated > 0 {
        persist_store(&store, &dump)
            .context("persisting store after escalation")?;
    }
    Ok(escalated)
}

/// Original sweep call — supersedes with a `verifier`-targeted twin.
/// Thin wrapper over the role-parameterised version below.
fn supersede_one(
    store: &oxigraph::store::Store,
    writer: &StreamWriter,
    old: &Feedback,
    stream_iri: &NamedNode,
) -> Result<()> {
    supersede_with_role_twin(store, writer, old, stream_iri, "verifier")
}

/// Mint a twin with the supplied `new_target_role` + transition the
/// original to `superseded` per ADR-024. Both writes go through a
/// single `StreamWriter` so failures (SHACL violation, bad IRI)
/// propagate before any partial state lands.
fn supersede_with_role_twin(
    store: &oxigraph::store::Store,
    writer: &StreamWriter,
    old: &Feedback,
    stream_iri: &NamedNode,
    new_target_role: &str,
) -> Result<()> {
    // Step 1: mint the corrected twin. Same source artifact + evidence
    // + source session, but `targetRole` is replaced so the dispatch
    // gate routes the twin to the new worker next iteration.
    let twin_iri = NamedNode::new_unchecked(format!(
        "urn:dec:feedback:{}",
        Uuid::new_v4()
    ));
    let twin = Feedback {
        iri: twin_iri.clone(),
        class: old.class.clone(),
        severity: Severity::Error,
        target_role: new_target_role.to_string(),
        evidence: old.evidence.clone(),
        recommendation: old.recommendation.clone(),
        lifecycle_state: LifecycleState::Produced.as_str().to_string(),
        source_session: old.source_session.clone(),
        source_artifact: old.source_artifact.clone(),
        addressing_artifact: None,
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: stream_iri.clone(),
    };
    writer
        .commit(Mutation::insert(twin.to_quads(orchestration_graph())))
        .with_context(|| format!("commit twin {twin_iri}"))?;

    // Step 2: transition the original to `superseded` with
    // `dec:supersededBy = <twin>`. The transition helper validates the
    // produced → superseded edge per ADR-024.
    let g: GraphName = orchestration_graph().into_owned().into();
    let evidence = vec![Quad::new(
        old.iri.clone(),
        NamedNodeRef::new_unchecked(crate::core::vocab::IRI_DEC_SUPERSEDED_BY).into_owned(),
        twin_iri.clone(),
        g.clone(),
    )];
    // The Lifecycle helper expects a non-empty rationale string for
    // some transitions but `superseded` only needs the supersededBy
    // quad we pass via `evidence`.
    let _ = Literal::new_simple_literal("");
    apply_transition(
        store,
        writer,
        &old.iri,
        LifecycleState::Superseded,
        evidence,
        orchestration_graph(),
    )
    .map_err(|e| anyhow::anyhow!("transitioning to superseded: {e}"))?;
    Ok(())
}
