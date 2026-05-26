//! Graph-accepted auto-dispatch subscription (FT-100).
//!
//! Fires when a `dec:VerificationGraph` lands in the store. Resolves the
//! configured envs, consults the dedup ledger, emits one
//! `dec:VerifyGraphRunDispatchEvent` per `(graph, env)` tuple, then
//! invokes `core::verify::runner::run_graph` to actually run the graph
//! and persist a `dec:VerificationGraphResult`. A
//! `verify-graph-runner` session is opened per dispatch with
//! `prov:wasInformedBy` the triggering event.
//!
//! Per the slice-level SDP convention, this module lives in `core/` and
//! is consumed by feature slices that trigger the natural events
//! (graph create / step add) plus the hidden test trigger in
//! `cli::trigger`.

pub mod config;
pub mod ledger;

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use thiserror::Error;

use crate::core::ontology::verification_graph::{from_turtle as graph_from_turtle, VerificationGraph};
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::stream_writer::StreamWriter;
use crate::core::verify::runner::{run_graph, RunGraphRequest, RunnerError, TriggerKind};
use crate::core::vocab::{
    aggregate_verdict_pred, code_change_pred, emitted_at, event_class, in_stream,
    orchestration_graph, run_activity_pred, target_role, trigger_kind_pred,
    verify_graph_ref, verify_graph_run_dispatch_event_class,
    EVENT_CLASS_VERIFY_GRAPH_RUN_DISPATCH, IRI_DEC_EVENT, IRI_DEC_SESSION,
    IRI_DEC_VERIFY_GRAPH_PREFIX, IRI_PROV_WAS_INFORMED_BY,
    SESSION_ROLE_VERIFY_GRAPH_RUNNER, TRIGGER_KIND_GRAPH_ACCEPTED,
    VERIFY_GRAPH_RUNNER_TARGET_ROLE,
};

pub use config::{
    load_from_workdir as load_config, parse_from_str as parse_config_from_str,
    GraphAcceptedDispatchConfig, DEFAULT_DEDUP_TTL_SECONDS, ENV_WILDCARD,
};
pub use ledger::{
    entry_iri as ledger_entry_iri, get_entry as ledger_get_entry,
    record_dispatch as ledger_record, within_ttl as ledger_within_ttl, LedgerEntry, LedgerError,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";
const PROV_ENDED_AT_TIME: &str = "http://www.w3.org/ns/prov#endedAtTime";
const DEC_STATUS: &str = "https://decision-cli.dev/ns#status";
const DEC_FEATURE_ID: &str = "https://decision-cli.dev/ns#featureId";
const DEC_ROLE: &str = "https://decision-cli.dev/ns#role";

/// Stable IRI of the seeded subscription. Tests assert presence by this IRI.
pub const SUBSCRIPTION_IRI: &str =
    "https://decision-cli.dev/ns/subscription/graph-accepted-dispatch";

/// Opaque handler tag the harness binds.
pub const HANDLER: &str = "graph-accepted-dispatch";

/// Raw seed TTL embedded via `include_str!`.
pub const SEED_TTL: &str = include_str!("../seeds/graph_accepted_dispatch.ttl");

/// Outcome of a single `dispatch_for_graph` call.
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    /// True when the handler was disabled by config.
    pub disabled: bool,
    /// `(graph_short, env_short)` pairs that were dispatched to the runner.
    pub dispatched: Vec<DispatchedTuple>,
    /// `(graph_short, env_short)` pairs that were dedup-skipped.
    pub skipped_dedup: Vec<(String, String)>,
    /// `(env_short, error)` for envs that could not be resolved.
    pub env_errors: Vec<(String, String)>,
}

/// One dispatched `(graph, env)` tuple with runner outcome.
#[derive(Debug, Clone)]
pub struct DispatchedTuple {
    /// Graph IRI (full).
    pub graph_iri: String,
    /// Graph short id.
    pub graph_short: String,
    /// Env IRI (full).
    pub env_iri: String,
    /// Env short id.
    pub env_short: String,
    /// IRI of the emitted `VerifyGraphRunDispatchEvent`.
    pub event_iri: NamedNode,
    /// IRI of the opened `verify-graph-runner` Session.
    pub session_iri: NamedNode,
    /// IRI of the resulting `VerificationGraphResult`, when the runner succeeded.
    pub result_iri: Option<NamedNode>,
    /// Verdict from the runner.
    pub verdict: Option<String>,
}

/// Errors surfaced by the handler.
#[derive(Debug, Error)]
pub enum GraphAcceptedDispatchError {
    /// Could not load or query the orchestration store.
    #[error("store unreachable: {0}")]
    Store(String),
    /// Could not load the graph artifact from disk.
    #[error("graph not found: {graph}")]
    GraphNotFound {
        /// Graph short id or IRI.
        graph: String,
    },
    /// IRI mint failure.
    #[error("invalid IRI: {0}")]
    IriMint(String),
    /// StreamWriter commit failure.
    #[error("commit failed: {0}")]
    Commit(String),
    /// Ledger I/O failure.
    #[error("ledger: {0}")]
    Ledger(#[source] LedgerError),
    /// Loading active scope failed.
    #[error("scope: {0}")]
    Scope(String),
}

impl From<LedgerError> for GraphAcceptedDispatchError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

/// Dispatch the runner for `graph_id` per the per-stream config. Top-level
/// entry point — called by `verify_step_add` (natural trigger), by
/// `verify_graph_new` (cold authoring path), and by the hidden
/// `dec _dispatch graph-accepted` command for tests.
pub fn dispatch_for_graph(
    workdir: &Path,
    graph_id: &str,
) -> Result<DispatchOutcome, GraphAcceptedDispatchError> {
    let cfg = load_config(workdir);
    if !cfg.enabled {
        return Ok(DispatchOutcome {
            disabled: true,
            dispatched: Vec::new(),
            skipped_dedup: Vec::new(),
            env_errors: Vec::new(),
        });
    }
    let graph = load_graph(workdir, graph_id)?;
    let envs = resolve_target_envs(&cfg, &graph);
    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();

    let mut outcome = DispatchOutcome {
        disabled: false,
        dispatched: Vec::new(),
        skipped_dedup: Vec::new(),
        env_errors: Vec::new(),
    };

    for env in envs {
        if !env_exists(workdir, &env.short) {
            outcome
                .env_errors
                .push((env.short.clone(), "env catalog entry missing".to_string()));
            tracing::warn!(target: "graph_accepted_dispatch", env = %env.short, "skipping dispatch: env not in catalog");
            continue;
        }
        // Open scope/store/writer once per env.
        let dump_path = orchestration_dump_path(workdir);
        let store = load_store_from_dump(&dump_path)
            .map_err(|e| GraphAcceptedDispatchError::Store(format!("{e:#}")))?;
        let store = Arc::new(store);
        let scope = ActiveScope::load(workdir)
            .map_err(|e| GraphAcceptedDispatchError::Scope(format!("{e:#}")))?;
        let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| {
            GraphAcceptedDispatchError::IriMint(format!("active stream iri: {e}"))
        })?;
        let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
            .map_err(|e| GraphAcceptedDispatchError::Commit(format!("opening writer: {e}")))?;

        if ledger::within_ttl(&store, graph.id.as_str(), env.iri.as_str(), cfg.dedup_ttl_seconds, now)? {
            outcome
                .skipped_dedup
                .push((graph.id_str().to_string(), env.short.clone()));
            continue;
        }

        // Emit the dispatch event.
        let event_iri = mint_event_iri()?;
        let dispatch_quads = build_dispatch_event_quads(
            &event_iri,
            &graph,
            &env.iri,
            None,
            &now_rfc3339,
            TRIGGER_KIND_GRAPH_ACCEPTED,
            None,
        );
        let mutation = Mutation::insert(dispatch_quads.iter().cloned())
            .with_cause("FT-100 emit graph-accepted dispatch event");
        writer
            .commit(mutation)
            .map_err(|e| GraphAcceptedDispatchError::Commit(format!("{e:#}")))?;

        // Run the graph runner. Errors from the runner are non-fatal: a
        // missing graph or safety violation has already produced a
        // rejected VGR; record the runner's response (if any) and move on.
        let run_activity = mint_run_activity(&graph.id_str(), &env.short);
        let runner_req = RunGraphRequest {
            graph: graph.id.clone(),
            triggered_by: TriggerKind::GraphAccepted,
            capture_bindings: std::collections::HashMap::new(),
            run_activity: run_activity.clone(),
            workdir: workdir.to_path_buf(),
        };
        let runner_result = run_graph(&runner_req);
        let (result_iri, verdict, completed) = match &runner_result {
            Ok(resp) => (
                Some(resp.result.clone()),
                Some(resp.verdict.as_str().to_string()),
                true,
            ),
            Err(RunnerError::SafetyViolation { .. }) => (None, Some("rejected".to_string()), true),
            Err(e) => {
                tracing::warn!(target: "graph_accepted_dispatch", err = %e, "runner returned non-fatal error");
                (None, Some("failed".to_string()), false)
            }
        };

        // Reload the store post-runner so the session writes see the
        // runner's commits.
        let store_after = load_store_from_dump(&orchestration_dump_path(workdir))
            .map_err(|e| GraphAcceptedDispatchError::Store(format!("post-run load: {e:#}")))?;
        let store_after = Arc::new(store_after);
        let scope_after = ActiveScope::load(workdir)
            .map_err(|e| GraphAcceptedDispatchError::Scope(format!("{e:#}")))?;
        let stream_iri_after = NamedNode::new(&scope_after.stream_iri).map_err(|e| {
            GraphAcceptedDispatchError::IriMint(format!("post-run stream iri: {e}"))
        })?;
        let writer_after = StreamWriter::open(Arc::clone(&store_after), stream_iri_after)
            .map_err(|e| GraphAcceptedDispatchError::Commit(format!("opening writer: {e}")))?;

        let session_iri = mint_session_iri(&graph.id_str(), &env.short);
        let session_quads = build_session_quads(
            &session_iri,
            &event_iri,
            &run_activity,
            graph.id_str(),
            &env.short,
            &now_rfc3339,
            if completed { "completed" } else { "failed" },
            result_iri.as_ref(),
            verdict.as_deref(),
        );
        let session_mutation = Mutation::insert(session_quads.iter().cloned())
            .with_cause("FT-100 verify-graph-runner session");
        writer_after
            .commit(session_mutation)
            .map_err(|e| GraphAcceptedDispatchError::Commit(format!("session: {e:#}")))?;

        // Record the ledger AFTER successful emission.
        ledger::record_dispatch(
            &writer_after,
            &store_after,
            graph.id.as_str(),
            env.iri.as_str(),
            &now_rfc3339,
        )?;

        persist_store(&store_after, &orchestration_dump_path(workdir))
            .map_err(|e| GraphAcceptedDispatchError::Commit(format!("persist: {e:#}")))?;

        outcome.dispatched.push(DispatchedTuple {
            graph_iri: graph.id.as_str().to_string(),
            graph_short: graph.id_str().to_string(),
            env_iri: env.iri.as_str().to_string(),
            env_short: env.short.clone(),
            event_iri,
            session_iri,
            result_iri,
            verdict,
        });
    }

    Ok(outcome)
}

/// Build the canonical quad set that seeds the subscription into the
/// `oxi-events:subscriptions` named graph. Mirrors FT-050's pattern.
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
                "PREFIX dec: <https://decision-cli.dev/ns#>\nSELECT ?graph WHERE { ?graph a dec:VerificationGraph . }",
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
                "verify-graph-runner auto-dispatch on VerificationGraph accept (FT-100)",
            ),
            subs_graph,
        ),
    ]
}

struct ResolvedEnv {
    iri: NamedNode,
    short: String,
}

fn resolve_target_envs(
    cfg: &GraphAcceptedDispatchConfig,
    graph: &VerificationGraph,
) -> Vec<ResolvedEnv> {
    use crate::core::vocab::IRI_DEC_ENV_PREFIX;
    let env_iri = graph.environment.clone();
    let env_short = env_iri
        .as_str()
        .strip_prefix(IRI_DEC_ENV_PREFIX)
        .unwrap_or(env_iri.as_str())
        .to_string();
    if cfg.envs_use_wildcard() {
        return vec![ResolvedEnv {
            iri: env_iri,
            short: env_short,
        }];
    }
    let mut out = Vec::new();
    for cfg_env in &cfg.envs {
        // Only dispatch against the graph's env if it's allowed by the config.
        if cfg_env == &env_short || cfg_env == env_iri.as_str() {
            out.push(ResolvedEnv {
                iri: env_iri.clone(),
                short: env_short.clone(),
            });
        }
    }
    out
}

fn env_exists(workdir: &Path, env_short: &str) -> bool {
    let path = workdir
        .join(".dec")
        .join("verify")
        .join("env")
        .join(format!("{env_short}.ttl"));
    path.is_file()
}

fn load_graph(
    workdir: &Path,
    graph_id: &str,
) -> Result<VerificationGraph, GraphAcceptedDispatchError> {
    let id = graph_id
        .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
        .unwrap_or(graph_id);
    let path = workdir
        .join(".dec")
        .join("verify")
        .join("graph")
        .join(format!("{id}.ttl"));
    if !path.is_file() {
        return Err(GraphAcceptedDispatchError::GraphNotFound {
            graph: graph_id.to_string(),
        });
    }
    graph_from_turtle(&path).map_err(|e| {
        GraphAcceptedDispatchError::Store(format!("parsing graph turtle: {e}"))
    })
}

fn mint_event_iri() -> Result<NamedNode, GraphAcceptedDispatchError> {
    let uuid = uuid::Uuid::new_v4();
    NamedNode::new(format!(
        "https://decision-cli.dev/ns/event/verify-graph-run-dispatch/{uuid}"
    ))
    .map_err(|e| GraphAcceptedDispatchError::IriMint(e.to_string()))
}

fn mint_session_iri(graph_short: &str, env_short: &str) -> NamedNode {
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

/// Build the canonical quad set for a `dec:VerifyGraphRunDispatchEvent`.
/// `code_change`, when present, links the event to a `dec:CodeChange` IRI
/// (used by the code-change-committed handler for shared event shape).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn build_dispatch_event_quads(
    event_iri: &NamedNode,
    graph: &VerificationGraph,
    env_iri: &NamedNode,
    informed_by: Option<&NamedNode>,
    emitted_at_rfc3339: &str,
    trigger_kind_literal: &str,
    code_change_iri: Option<&NamedNode>,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let event_class_iri = NamedNodeRef::new_unchecked(IRI_DEC_EVENT).into_owned();
    let prov_informed = NamedNodeRef::new_unchecked(IRI_PROV_WAS_INFORMED_BY).into_owned();
    let mut quads = vec![
        Quad::new(
            event_iri.clone(),
            rdf_type.clone(),
            event_class_iri,
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            rdf_type,
            verify_graph_run_dispatch_event_class().into_owned(),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            event_class().into_owned(),
            Literal::new_simple_literal(EVENT_CLASS_VERIFY_GRAPH_RUN_DISPATCH),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            target_role().into_owned(),
            Literal::new_simple_literal(VERIFY_GRAPH_RUNNER_TARGET_ROLE),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            verify_graph_ref().into_owned(),
            graph.id.clone(),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            NamedNodeRef::new_unchecked(crate::core::vocab::IRI_DEC_ENVIRONMENT).into_owned(),
            env_iri.clone(),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            trigger_kind_pred().into_owned(),
            Literal::new_simple_literal(trigger_kind_literal),
            g.clone(),
        ),
        Quad::new(
            event_iri.clone(),
            emitted_at().into_owned(),
            Literal::new_simple_literal(emitted_at_rfc3339),
            g.clone(),
        ),
    ];
    if let Some(by) = informed_by {
        quads.push(Quad::new(
            event_iri.clone(),
            prov_informed,
            by.clone(),
            g.clone(),
        ));
    }
    if let Some(cc) = code_change_iri {
        quads.push(Quad::new(
            event_iri.clone(),
            code_change_pred().into_owned(),
            cc.clone(),
            g,
        ));
    }
    quads
}

/// Build a per-`(graph, env)` `verify-graph-runner` Session.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn build_session_quads(
    session_iri: &NamedNode,
    event_iri: &NamedNode,
    run_activity: &NamedNode,
    feature_or_graph_short: &str,
    env_short: &str,
    started_at_rfc3339: &str,
    status: &str,
    result_iri: Option<&NamedNode>,
    verdict: Option<&str>,
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
    let env_pred = NamedNodeRef::new_unchecked(crate::core::vocab::IRI_DEC_ENVIRONMENT).into_owned();
    let prov_informed = NamedNodeRef::new_unchecked(IRI_PROV_WAS_INFORMED_BY).into_owned();
    let _ = in_stream();

    let mut quads = vec![
        Quad::new(session_iri.clone(), rdf_type.clone(), session_cls, g.clone()),
        Quad::new(session_iri.clone(), rdf_type, activity_cls, g.clone()),
        Quad::new(
            session_iri.clone(),
            status_pred,
            Literal::new_simple_literal(status),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            role_pred,
            Literal::new_simple_literal(SESSION_ROLE_VERIFY_GRAPH_RUNNER),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            feature_id_pred,
            Literal::new_simple_literal(feature_or_graph_short),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            env_pred,
            Literal::new_simple_literal(env_short),
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
            event_iri.clone(),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            run_activity_pred().into_owned(),
            run_activity.clone(),
            g.clone(),
        ),
    ];
    if let Some(rid) = result_iri {
        quads.push(Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#outputRef").into_owned(),
            rid.clone(),
            g.clone(),
        ));
    }
    if let Some(v) = verdict {
        quads.push(Quad::new(
            session_iri.clone(),
            aggregate_verdict_pred().into_owned(),
            Literal::new_simple_literal(v),
            g,
        ));
    }
    quads
}
