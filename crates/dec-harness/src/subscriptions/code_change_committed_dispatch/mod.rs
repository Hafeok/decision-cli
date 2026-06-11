//! Code-change-committed auto-dispatch subscription (FT-100).
//!
//! Fires when an implementer's `dec:CodeChange` lands (FT-017). The
//! handler enumerates every covering `dec:VerificationGraph` for the
//! targeted feature, schedules a per-`(graph, env)` runner dispatch,
//! composes the per-graph results through FT-097's aggregation rule, and
//! writes one aggregate session per CodeChange with
//! `prov:wasInformedBy` the triggering `dec:CodeChangeCommitted` event.
//!
//! If the aggregate verdict is `rejected`, the handler emits a single
//! feature-level `dec:Feedback { class: "regression" }` against the
//! feature (unless suppressed by `feedback_routes = "suppress"`).
//! If there is no covering graph, it emits a `dec:Feedback { class: "gap" }`
//! and an aggregate session with `verdict = rejected`.

pub mod config;
pub mod enumerate;
pub mod ledger;

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use thiserror::Error;

use crate::verify::aggregate::{aggregate_verdict, AggregationTarget};
use crate::verify::coverage::feature_resolver::{feature_iri_for, resolve_feature_tc_iris};
use crate::verify::runner::{
    run_graph, RunGraphRequest, RunGraphResponse, RunnerError, TriggerKind,
};
use dec_graph::ontology::verdict::Verdict;
use dec_graph::ontology::verification_graph::from_turtle as graph_from_turtle;
use dec_graph::ontology::verification_result::VerificationGraphResult;
use dec_graph::scope::ActiveScope;
use dec_graph::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use dec_graph::stream_writer::StreamWriter;
use dec_ontology::vocab::{
    aggregate_verdict_pred, code_change_pred, emitted_at, event_class, in_stream,
    orchestration_graph, partial_failure_reasons_pred, target_role, trigger_kind_pred,
    verify_graph_run_dispatch_event_class, EVENT_CLASS_VERIFY_GRAPH_RUN_DISPATCH, IRI_DEC_EVENT,
    IRI_DEC_SESSION, IRI_PROV_WAS_INFORMED_BY, SESSION_ROLE_VERIFY_GRAPH_RUNNER_AGGREGATE,
    TRIGGER_KIND_CODE_CHANGE_COMMITTED, VERIFY_GRAPH_RUNNER_TARGET_ROLE,
};

pub use config::{
    load_from_workdir as load_config, parse_from_str as parse_config_from_str,
    CodeChangeCommittedDispatchConfig, DEFAULT_DEDUP_TTL_SECONDS, ENV_WILDCARD,
};
pub use enumerate::{enumerate_covering_tuples, EnumerateError, GraphTuple};
pub use ledger::{
    entry_iri as ledger_entry_iri, get_entry as ledger_get_entry, record_dispatch as ledger_record,
    within_ttl as ledger_within_ttl, LedgerEntry, LedgerError,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";
const PROV_ENDED_AT_TIME: &str = "http://www.w3.org/ns/prov#endedAtTime";
const DEC_STATUS: &str = "https://decision-cli.dev/ns#status";
const DEC_FEATURE_ID: &str = "https://decision-cli.dev/ns#featureId";
const DEC_ROLE: &str = "https://decision-cli.dev/ns#role";
const DEC_RATIONALE: &str = "https://decision-cli.dev/ns#rationale";
const DEC_TARGET: &str = "https://decision-cli.dev/ns#target";
const DEC_CLASS: &str = "https://decision-cli.dev/ns#class";

/// Stable IRI of the seeded subscription.
pub const SUBSCRIPTION_IRI: &str =
    "https://decision-cli.dev/ns/subscription/code-change-committed-dispatch";

/// Opaque handler tag the harness binds.
pub const HANDLER: &str = "code-change-committed-dispatch";

/// Raw seed TTL embedded via `include_str!`.
pub const SEED_TTL: &str = include_str!("../seeds/code_change_committed_dispatch.ttl");

/// Outcome of a single `dispatch_for_code_change` call.
#[derive(Debug, Clone)]
pub struct AggregateOutcome {
    /// True when handler was disabled.
    pub disabled: bool,
    /// IRI of the aggregate session.
    pub aggregate_session: Option<NamedNode>,
    /// Aggregate verdict literal.
    pub aggregate_verdict: Option<String>,
    /// Per-graph dispatched tuples.
    pub per_graph: Vec<PerGraphDispatch>,
    /// Aggregate feedback IRIs emitted (regression / gap).
    pub aggregate_feedback: Vec<NamedNode>,
    /// True iff the handler observed a "coverage gap" (no covering graph).
    pub coverage_gap: bool,
    /// True iff dedup skipped this commit.
    pub skipped_dedup: bool,
}

/// One per-graph dispatch line.
#[derive(Debug, Clone)]
pub struct PerGraphDispatch {
    /// Graph short id.
    pub graph_short: String,
    /// Env short id.
    pub env_short: String,
    /// Session IRI.
    pub session: NamedNode,
    /// Optional result IRI.
    pub result: Option<NamedNode>,
    /// Verdict literal.
    pub verdict: Option<String>,
}

/// Handler error envelope.
#[derive(Debug, Error)]
pub enum CodeChangeDispatchError {
    /// Store load failed.
    #[error("store unreachable: {0}")]
    Store(String),
    /// Feature artifact missing or unreadable.
    #[error("feature not found: {0}")]
    FeatureNotFound(String),
    /// IRI mint failure.
    #[error("invalid IRI: {0}")]
    IriMint(String),
    /// StreamWriter commit failure.
    #[error("commit failed: {0}")]
    Commit(String),
    /// Ledger I/O failure.
    #[error("ledger: {0}")]
    Ledger(#[source] LedgerError),
    /// Scope load failed.
    #[error("scope: {0}")]
    Scope(String),
}

impl From<LedgerError> for CodeChangeDispatchError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

/// Dispatch the runner for every covering `(graph, env)` tuple of
/// `feature_id`, then write an aggregate session.
pub fn dispatch_for_code_change(
    workdir: &Path,
    feature_id: &str,
    code_change_iri: &str,
) -> Result<AggregateOutcome, CodeChangeDispatchError> {
    let cfg = load_config(workdir);
    if !cfg.enabled {
        return Ok(AggregateOutcome {
            disabled: true,
            aggregate_session: None,
            aggregate_verdict: None,
            per_graph: Vec::new(),
            aggregate_feedback: Vec::new(),
            coverage_gap: false,
            skipped_dedup: false,
        });
    }

    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();

    // Ledger dedup check.
    {
        let store = load_store_from_dump(&orchestration_dump_path(workdir))
            .map_err(|e| CodeChangeDispatchError::Store(format!("{e:#}")))?;
        let store = Arc::new(store);
        if ledger::within_ttl(
            &store,
            code_change_iri,
            feature_id,
            cfg.dedup_ttl_seconds,
            now,
        )? {
            return Ok(AggregateOutcome {
                disabled: false,
                aggregate_session: None,
                aggregate_verdict: None,
                per_graph: Vec::new(),
                aggregate_feedback: Vec::new(),
                coverage_gap: false,
                skipped_dedup: true,
            });
        }
    }

    let feature_iri = feature_iri_for(feature_id);
    let tcs = resolve_feature_tc_iris(workdir, feature_id)
        .map_err(|_| CodeChangeDispatchError::FeatureNotFound(feature_id.to_string()))?;
    let env_filter = if cfg.envs_use_wildcard() {
        None
    } else {
        cfg.envs.first().map(|s| s.as_str())
    };
    let tuples = enumerate_covering_tuples(workdir, &feature_iri, &tcs, env_filter)
        .map_err(|e| CodeChangeDispatchError::Store(format!("enumerate: {e}")))?;

    let code_change_node = NamedNode::new(code_change_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("code_change iri: {e}")))?;

    // Coverage gap: no covering graphs.
    if tuples.is_empty() {
        let outcome = persist_coverage_gap_aggregate(
            workdir,
            feature_id,
            &feature_iri,
            &code_change_node,
            &now_rfc3339,
            &cfg,
        )?;
        return Ok(outcome);
    }

    // Per-`(graph, env)` dispatch loop. Each loop iteration emits an
    // event + runs the runner + opens a runner session. The aggregate
    // session is built after the loop.
    let mut per_graph_dispatches: Vec<PerGraphDispatch> = Vec::with_capacity(tuples.len());
    let mut runner_results: Vec<VerificationGraphResult> = Vec::new();
    let mut partial_failures: Vec<String> = Vec::new();

    for tuple in &tuples {
        match dispatch_single_tuple(workdir, tuple, &code_change_node, feature_id, &now_rfc3339) {
            Ok((dispatch, result_artifact)) => {
                if let Some(ra) = result_artifact {
                    runner_results.push(ra);
                }
                per_graph_dispatches.push(dispatch);
            }
            Err(e) => {
                partial_failures.push(format!("{} ({}): {e}", tuple.graph_short, tuple.env_short));
            }
        }
    }

    // Aggregate verdict + session.
    let aggregate = aggregate_verdict(
        AggregationTarget::Feature {
            feature: feature_iri.clone(),
            tests: tcs.clone(),
        },
        &runner_results,
    );

    // Write aggregate session + ledger.
    let store = load_store_from_dump(&orchestration_dump_path(workdir))
        .map_err(|e| CodeChangeDispatchError::Store(format!("{e:#}")))?;
    let store = Arc::new(store);
    let scope =
        ActiveScope::load(workdir).map_err(|e| CodeChangeDispatchError::Scope(format!("{e:#}")))?;
    let stream_iri = NamedNode::new(&scope.stream_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("stream iri: {e}")))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("opening writer: {e}")))?;

    let agg_session_iri = mint_aggregate_session_iri(feature_id, code_change_iri);
    let agg_event_iri = mint_event_iri()?;
    let agg_quads = build_aggregate_session_quads(
        &agg_session_iri,
        &agg_event_iri,
        &code_change_node,
        feature_id,
        &aggregate.verdict.as_str().to_string(),
        &aggregate.rationale,
        &now_rfc3339,
        &per_graph_dispatches,
        &partial_failures,
    );
    let mutation = Mutation::insert(agg_quads.iter().cloned())
        .with_cause("FT-100 code-change-committed aggregate session");
    writer
        .commit(mutation)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("aggregate: {e:#}")))?;

    // Aggregate feedback (regression) if verdict is rejected.
    let mut feedback_iris = Vec::new();
    if matches!(aggregate.verdict, Verdict::Rejected) && !cfg.feedback_routes_suppress {
        let fb = persist_aggregate_feedback(
            &writer,
            feature_id,
            &feature_iri,
            &agg_session_iri,
            "regression",
            &format!("code-change {code_change_iri} produced rejected aggregate for {feature_id}"),
            &now_rfc3339,
        )?;
        feedback_iris.push(fb);
    }

    ledger::record_dispatch(&writer, &store, code_change_iri, feature_id, &now_rfc3339)?;
    persist_store(&store, &orchestration_dump_path(workdir))
        .map_err(|e| CodeChangeDispatchError::Commit(format!("persist: {e:#}")))?;

    Ok(AggregateOutcome {
        disabled: false,
        aggregate_session: Some(agg_session_iri),
        aggregate_verdict: Some(aggregate.verdict.as_str().to_string()),
        per_graph: per_graph_dispatches,
        aggregate_feedback: feedback_iris,
        coverage_gap: false,
        skipped_dedup: false,
    })
}

fn dispatch_single_tuple(
    workdir: &Path,
    tuple: &GraphTuple,
    code_change: &NamedNode,
    feature_id: &str,
    now_rfc3339: &str,
) -> Result<(PerGraphDispatch, Option<VerificationGraphResult>), CodeChangeDispatchError> {
    let store = load_store_from_dump(&orchestration_dump_path(workdir))
        .map_err(|e| CodeChangeDispatchError::Store(format!("{e:#}")))?;
    let store = Arc::new(store);
    let scope =
        ActiveScope::load(workdir).map_err(|e| CodeChangeDispatchError::Scope(format!("{e:#}")))?;
    let stream_iri = NamedNode::new(&scope.stream_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("stream iri: {e}")))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("opening writer: {e}")))?;

    // Emit per-graph dispatch event.
    let graph_node = NamedNode::new(&tuple.graph_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("graph iri: {e}")))?;
    let env_node = NamedNode::new(&tuple.env_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("env iri: {e}")))?;
    let event_iri = mint_event_iri()?;
    // Load graph to obtain the VerificationGraph for quad-building.
    let graph_path = workdir
        .join(".dec")
        .join("verify")
        .join("graph")
        .join(format!("{}.ttl", tuple.graph_short));
    let graph = graph_from_turtle(&graph_path)
        .map_err(|e| CodeChangeDispatchError::Store(format!("parsing graph: {e}")))?;

    let dispatch_quads = crate::subscriptions::graph_accepted_dispatch::build_dispatch_event_quads(
        &event_iri,
        &graph,
        &env_node,
        None,
        now_rfc3339,
        TRIGGER_KIND_CODE_CHANGE_COMMITTED,
        Some(code_change),
    );
    let mutation = Mutation::insert(dispatch_quads.iter().cloned())
        .with_cause("FT-100 code-change-committed dispatch event");
    writer
        .commit(mutation)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("{e:#}")))?;

    // Run the runner.
    let run_activity = mint_run_activity(&tuple.graph_short, &tuple.env_short);
    let runner_req = RunGraphRequest {
        graph: graph_node,
        triggered_by: TriggerKind::CodeChangeCommitted {
            code_change: code_change.clone(),
        },
        capture_bindings: std::collections::HashMap::new(),
        run_activity: run_activity.clone(),
        workdir: workdir.to_path_buf(),
    };
    let (result_iri, verdict, completed, result_artifact) = match run_graph(&runner_req) {
        Ok(RunGraphResponse {
            result,
            verdict,
            result_artifact,
            ..
        }) => (
            Some(result),
            Some(verdict.as_str().to_string()),
            true,
            Some(result_artifact),
        ),
        Err(RunnerError::SafetyViolation { .. }) => {
            (None, Some("rejected".to_string()), true, None)
        }
        Err(e) => {
            tracing::warn!(target: "code_change_committed_dispatch", err = %e, "runner returned non-fatal error");
            (None, Some("failed".to_string()), false, None)
        }
    };

    // Open per-graph session after the runner committed its VGR.
    let store_after = load_store_from_dump(&orchestration_dump_path(workdir))
        .map_err(|e| CodeChangeDispatchError::Store(format!("post-run load: {e:#}")))?;
    let store_after = Arc::new(store_after);
    let scope_after =
        ActiveScope::load(workdir).map_err(|e| CodeChangeDispatchError::Scope(format!("{e:#}")))?;
    let stream_iri_after = NamedNode::new(&scope_after.stream_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("post-run stream iri: {e}")))?;
    let writer_after = StreamWriter::open(Arc::clone(&store_after), stream_iri_after)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("opening writer: {e}")))?;

    let session_iri = mint_per_graph_session_iri(&tuple.graph_short, &tuple.env_short);
    let session_quads = crate::subscriptions::graph_accepted_dispatch::build_session_quads(
        &session_iri,
        &event_iri,
        &run_activity,
        feature_id,
        &tuple.env_short,
        now_rfc3339,
        if completed { "completed" } else { "failed" },
        result_iri.as_ref(),
        verdict.as_deref(),
    );
    let mutation = Mutation::insert(session_quads.iter().cloned())
        .with_cause("FT-100 per-graph verify-graph-runner session");
    writer_after
        .commit(mutation)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("per-graph session: {e:#}")))?;

    persist_store(&store_after, &orchestration_dump_path(workdir))
        .map_err(|e| CodeChangeDispatchError::Commit(format!("persist: {e:#}")))?;

    Ok((
        PerGraphDispatch {
            graph_short: tuple.graph_short.clone(),
            env_short: tuple.env_short.clone(),
            session: session_iri,
            result: result_iri,
            verdict,
        },
        result_artifact,
    ))
}

fn persist_coverage_gap_aggregate(
    workdir: &Path,
    feature_id: &str,
    feature_iri: &str,
    code_change: &NamedNode,
    now_rfc3339: &str,
    cfg: &CodeChangeCommittedDispatchConfig,
) -> Result<AggregateOutcome, CodeChangeDispatchError> {
    let store = load_store_from_dump(&orchestration_dump_path(workdir))
        .map_err(|e| CodeChangeDispatchError::Store(format!("{e:#}")))?;
    let store = Arc::new(store);
    let scope =
        ActiveScope::load(workdir).map_err(|e| CodeChangeDispatchError::Scope(format!("{e:#}")))?;
    let stream_iri = NamedNode::new(&scope.stream_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("stream iri: {e}")))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("opening writer: {e}")))?;

    let agg_session_iri = mint_aggregate_session_iri(feature_id, code_change.as_str());
    let agg_event_iri = mint_event_iri()?;
    let rationale =
        "no covering verification graphs for feature; chain-integrity gate likely waived"
            .to_string();
    let agg_quads = build_aggregate_session_quads(
        &agg_session_iri,
        &agg_event_iri,
        code_change,
        feature_id,
        "rejected",
        &rationale,
        now_rfc3339,
        &[],
        &[],
    );
    let mutation = Mutation::insert(agg_quads.iter().cloned())
        .with_cause("FT-100 coverage-gap aggregate session");
    writer
        .commit(mutation)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("agg: {e:#}")))?;

    let mut feedback_iris = Vec::new();
    if !cfg.feedback_routes_suppress {
        let fb = persist_aggregate_feedback(
            &writer,
            feature_id,
            feature_iri,
            &agg_session_iri,
            "gap",
            &format!("no covering verification graphs for feature {feature_id}"),
            now_rfc3339,
        )?;
        feedback_iris.push(fb);
    }

    ledger::record_dispatch(
        &writer,
        &store,
        code_change.as_str(),
        feature_id,
        now_rfc3339,
    )?;
    persist_store(&store, &orchestration_dump_path(workdir))
        .map_err(|e| CodeChangeDispatchError::Commit(format!("persist: {e:#}")))?;

    Ok(AggregateOutcome {
        disabled: false,
        aggregate_session: Some(agg_session_iri),
        aggregate_verdict: Some("rejected".to_string()),
        per_graph: Vec::new(),
        aggregate_feedback: feedback_iris,
        coverage_gap: true,
        skipped_dedup: false,
    })
}

fn persist_aggregate_feedback(
    writer: &StreamWriter,
    feature_id: &str,
    feature_iri: &str,
    session_iri: &NamedNode,
    class: &str,
    body: &str,
    _now_rfc3339: &str,
) -> Result<NamedNode, CodeChangeDispatchError> {
    use crate::feedback::artifact::{Feedback, Severity};
    let uuid = uuid::Uuid::new_v4();
    let fb_iri = NamedNode::new_unchecked(format!(
        "urn:dec:feedback/ft-100/{class}/{feature_id}/{uuid}"
    ));
    let feature_node = NamedNode::new(feature_iri)
        .map_err(|e| CodeChangeDispatchError::IriMint(format!("feature iri: {e}")))?;
    // The strict SHACL feedback class vocabulary is `defect`/`gap`/etc.
    // Map our rollup class to the closest existing vocab so the
    // mutation conforms; the operator-facing label is preserved via
    // the additional `dec:class` predicate.
    let (feedback_class_iri_value, target_role) = match class {
        "gap" => ("gap", "verify-graph-author"),
        // FT-100 aggregate regression: closest existing vocab is "defect".
        _ => ("defect", "code-writer"),
    };
    let feedback = Feedback {
        iri: fb_iri.clone(),
        class: feedback_class_iri_value.to_string(),
        severity: Severity::Error,
        target_role: target_role.to_string(),
        evidence: body.to_string(),
        recommendation: None,
        lifecycle_state: "produced".to_string(),
        source_session: session_iri.clone(),
        source_artifact: Some(feature_node.clone()),
        addressing_artifact: None,
        closed_by: None,
        rejection_reason: None,
        superseded_by: None,
        routed_at: None,
        receiving_session: None,
        disposition_override: None,
        disposition_rationale: None,
        in_stream: writer.active_stream().clone(),
    };
    let mut quads = feedback.to_quads(orchestration_graph());
    // Add the operator-facing rollup class + target predicate (extra
    // metadata; not SHACL-validated).
    let g: GraphName = orchestration_graph().into_owned().into();
    let class_pred = NamedNodeRef::new_unchecked(DEC_CLASS).into_owned();
    let target_pred = NamedNodeRef::new_unchecked(DEC_TARGET).into_owned();
    quads.push(Quad::new(
        fb_iri.clone(),
        class_pred,
        Literal::new_simple_literal(class),
        g.clone(),
    ));
    quads.push(Quad::new(fb_iri.clone(), target_pred, feature_node, g));
    let mutation = Mutation::insert(quads).with_cause("FT-100 aggregate feedback");
    writer
        .commit(mutation)
        .map_err(|e| CodeChangeDispatchError::Commit(format!("feedback: {e:#}")))?;
    Ok(fb_iri)
}

#[allow(clippy::too_many_arguments)]
fn build_aggregate_session_quads(
    session_iri: &NamedNode,
    informed_by_event: &NamedNode,
    code_change: &NamedNode,
    feature_id: &str,
    verdict_literal: &str,
    rationale: &str,
    started_at_rfc3339: &str,
    per_graph: &[PerGraphDispatch],
    partial_failures: &[String],
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let session_cls = NamedNodeRef::new_unchecked(IRI_DEC_SESSION).into_owned();
    let activity_cls = NamedNodeRef::new_unchecked(PROV_ACTIVITY).into_owned();
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME).into_owned();
    let ended_at = NamedNodeRef::new_unchecked(PROV_ENDED_AT_TIME).into_owned();
    let status_pred = NamedNodeRef::new_unchecked(DEC_STATUS).into_owned();
    let role_pred = NamedNodeRef::new_unchecked(DEC_ROLE).into_owned();
    let feature_id_pred = NamedNodeRef::new_unchecked(DEC_FEATURE_ID).into_owned();
    let rationale_pred = NamedNodeRef::new_unchecked(DEC_RATIONALE).into_owned();
    let prov_informed = NamedNodeRef::new_unchecked(IRI_PROV_WAS_INFORMED_BY).into_owned();
    let _ = in_stream();

    let mut quads = vec![
        Quad::new(
            session_iri.clone(),
            rdf_type.clone(),
            session_cls,
            g.clone(),
        ),
        Quad::new(session_iri.clone(), rdf_type, activity_cls, g.clone()),
        Quad::new(
            session_iri.clone(),
            status_pred,
            Literal::new_simple_literal("completed"),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            role_pred,
            Literal::new_simple_literal(SESSION_ROLE_VERIFY_GRAPH_RUNNER_AGGREGATE),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            feature_id_pred,
            Literal::new_simple_literal(feature_id),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            at_time,
            Literal::new_simple_literal(started_at_rfc3339),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            ended_at,
            Literal::new_simple_literal(started_at_rfc3339),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            prov_informed,
            informed_by_event.clone(),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            code_change_pred().into_owned(),
            code_change.clone(),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            aggregate_verdict_pred().into_owned(),
            Literal::new_simple_literal(verdict_literal),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            rationale_pred,
            Literal::new_simple_literal(rationale),
            g.clone(),
        ),
    ];
    for d in per_graph {
        if let Some(rid) = &d.result {
            quads.push(Quad::new(
                session_iri.clone(),
                NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#outputRef").into_owned(),
                rid.clone(),
                g.clone(),
            ));
        }
    }
    if !partial_failures.is_empty() {
        for pf in partial_failures {
            quads.push(Quad::new(
                session_iri.clone(),
                partial_failure_reasons_pred().into_owned(),
                Literal::new_simple_literal(pf.as_str()),
                g.clone(),
            ));
        }
    }
    // Emit the *triggering* event as part of the aggregate session's
    // own informed-by chain.
    quads.push(Quad::new(
        informed_by_event.clone(),
        rdf_type_owned(),
        NamedNodeRef::new_unchecked(IRI_DEC_EVENT).into_owned(),
        g.clone(),
    ));
    quads.push(Quad::new(
        informed_by_event.clone(),
        rdf_type_owned(),
        verify_graph_run_dispatch_event_class().into_owned(),
        g.clone(),
    ));
    quads.push(Quad::new(
        informed_by_event.clone(),
        event_class().into_owned(),
        Literal::new_simple_literal(EVENT_CLASS_VERIFY_GRAPH_RUN_DISPATCH),
        g.clone(),
    ));
    quads.push(Quad::new(
        informed_by_event.clone(),
        target_role().into_owned(),
        Literal::new_simple_literal(VERIFY_GRAPH_RUNNER_TARGET_ROLE),
        g.clone(),
    ));
    quads.push(Quad::new(
        informed_by_event.clone(),
        trigger_kind_pred().into_owned(),
        Literal::new_simple_literal(TRIGGER_KIND_CODE_CHANGE_COMMITTED),
        g.clone(),
    ));
    quads.push(Quad::new(
        informed_by_event.clone(),
        code_change_pred().into_owned(),
        code_change.clone(),
        g.clone(),
    ));
    quads.push(Quad::new(
        informed_by_event.clone(),
        emitted_at().into_owned(),
        Literal::new_simple_literal(started_at_rfc3339),
        g,
    ));
    quads
}

fn rdf_type_owned() -> NamedNode {
    NamedNodeRef::new_unchecked(RDF_TYPE).into_owned()
}

fn mint_event_iri() -> Result<NamedNode, CodeChangeDispatchError> {
    let uuid = uuid::Uuid::new_v4();
    NamedNode::new(format!(
        "https://decision-cli.dev/ns/event/verify-graph-run-dispatch/{uuid}"
    ))
    .map_err(|e| CodeChangeDispatchError::IriMint(e.to_string()))
}

fn mint_aggregate_session_iri(feature_id: &str, code_change_iri: &str) -> NamedNode {
    let uuid = uuid::Uuid::new_v4();
    // Use a hash of the code-change so the session IRI is stable-ish
    // across retries within the same window (per FT-100 §Idempotency).
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(code_change_iri.as_bytes());
    let hex: String = h
        .finalize()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect();
    NamedNode::new_unchecked(format!(
        "urn:dec:session/verify-graph-runner-aggregate/{feature_id}/{hex}/{uuid}"
    ))
}

fn mint_per_graph_session_iri(graph_short: &str, env_short: &str) -> NamedNode {
    let uuid = uuid::Uuid::new_v4();
    NamedNode::new_unchecked(format!(
        "urn:dec:session/verify-graph-runner/{graph_short}/{env_short}/{uuid}"
    ))
}

fn mint_run_activity(graph_short: &str, env_short: &str) -> NamedNode {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/activity/verify-graph-run/{graph_short}/{env_short}/ts-{nanos}"
    ))
}

/// Build the seed quad set for this subscription.
#[must_use]
pub fn seed_quads() -> Vec<Quad> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let sub = NamedNode::new_unchecked(SUBSCRIPTION_IRI);
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let sub_cls = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    let handler_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_HANDLER);
    let label_pred = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    vec![
        Quad::new(sub.clone(), rdf_type, sub_cls, subs_graph.clone()),
        Quad::new(
            sub.clone(),
            select_pred,
            Literal::new_simple_literal(
                "PREFIX dec: <https://decision-cli.dev/ns#>\nSELECT ?codeChange WHERE { ?codeChange a dec:CodeChange . }",
            ),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            mode_pred,
            Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_ASYNC),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            handler_pred,
            Literal::new_simple_literal(HANDLER),
            subs_graph.clone(),
        ),
        Quad::new(
            sub,
            label_pred,
            Literal::new_simple_literal(
                "verify-graph-runner auto-dispatch on CodeChange commit (FT-100)",
            ),
            subs_graph,
        ),
    ]
}
