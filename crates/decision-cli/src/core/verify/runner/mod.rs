//! Procedural executor that turns a `dec:VerificationGraph` into a
//! `dec:VerificationGraphResult` (FT-098 / ADR-028).
//!
//! Narrow public surface per FT-098 §Invariants: only [`run_graph`], the
//! request/response/error envelopes, and the [`StepKindHandler`] trait
//! (plus the six seed handlers) are exported. Internal modules are
//! `pub(crate)` so feature slices that consume the runner cannot reach
//! into its private state.
//!
//! Phase structure mirrors FT-098 §Behaviour:
//!
//! 1. Load graph + env; defensive op-gate (`runtime_safety`).
//! 2. Env setup (`env_lifecycle::setup`).
//! 3. Step loop with capture substitution + per-kind dispatch.
//! 4. Env teardown (`env_lifecycle::teardown`).
//! 5. Build VGR + persist via `StreamWriter`; emit per-failure feedback.

pub mod context;
mod env_lifecycle;
mod feedback;
pub mod kinds;
pub mod request;
mod runtime_safety;
mod trace_writer;

use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;

use crate::core::ontology::verdict::Verdict;
use crate::core::ontology::verification_env::{from_turtle as env_from_turtle, VerificationEnvironment};
use crate::core::ontology::verification_graph::{from_turtle as graph_from_turtle, VerificationGraph};
use crate::core::ontology::verification_result::{
    to_canonical_turtle, StepOutcome, VerificationGraphResult,
};
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::vocab::{verify_result_graph, IRI_DEC_ENV_PREFIX, IRI_DEC_VERIFY_GRAPH_PREFIX, IRI_DEC_VERIFY_RESULT_PREFIX};
use crate::core::StreamWriter;

use self::context::RunContext;
use self::kinds::{dispatch, StepRunTrace};

pub use self::kinds::{
    CaptureHandler, FileHandler, HttpHandler, ShellHandler, SparqlHandler, StepKindHandler,
    WaitForHandler,
};
pub use self::request::{
    RunGraphRequest, RunGraphResponse, RunnerError, StepOutcomeSummary, TriggerKind,
};

/// IRI used as `prov:wasAttributedTo` for runner-produced artifacts when
/// the caller does not specify a more specific agent.
const RUNNER_AGENT_IRI: &str = "https://decision-cli.dev/ns/agent/verify-graph-runner";

/// Execute a `VerificationGraph` against its declared environment and
/// persist a `VerificationGraphResult`.
///
/// See module documentation for the five-phase contract.
pub fn run_graph(req: &RunGraphRequest) -> Result<RunGraphResponse, RunnerError> {
    let runner_agent = NamedNode::new_unchecked(RUNNER_AGENT_IRI);
    // Phase 1: load + safety re-check.
    let graph = load_graph(&req.workdir, &req.graph)?;
    let env = load_env(&req.workdir, &graph)?;
    if let Err(safety_err) = runtime_safety::check(&graph, &env) {
        // Phase-1 abort path: persist a rejected VGR with empty traces.
        let safety_label = match &safety_err {
            RunnerError::SafetyViolation { step, op } => format!(
                "safety: step <{step}> requires op <{op}> not in env.allowedOps"
            ),
            _ => "safety: unknown op token rejected at runtime".to_string(),
        };
        persist_rejected(
            &req.workdir,
            &graph,
            &env,
            &req.run_activity,
            &runner_agent,
            safety_label,
        )?;
        return Err(safety_err);
    }

    // Phase 2: env setup (best-effort `dec:setup` script).
    let env_handle = env_lifecycle::setup(&req.workdir, &env);
    if let Some(setup_script) = &env.setup {
        if let Err(e) = run_setup(setup_script, &env_handle.dec_workdir) {
            env_lifecycle::teardown(&env_handle);
            persist_rejected(
                &req.workdir,
                &graph,
                &env,
                &req.run_activity,
                &runner_agent,
                format!("env setup failed before any step ran: {e}"),
            )?;
            return Err(RunnerError::EnvSetupFailed {
                exit_code: 1,
                stderr_excerpt: e,
            });
        }
    }

    // Phase 3: step loop.
    let started_at = self::kinds::iso_now_pub();
    let mut ctx = RunContext::new(
        req.workdir.clone(),
        env_handle.dec_workdir.clone(),
        env_handle.cleanup,
        req.capture_bindings.clone(),
        &graph.steps,
    );
    let mut traces: Vec<StepRunTrace> = Vec::with_capacity(graph.steps.len());
    for (i, step) in graph.steps.iter().enumerate() {
        if let Some(halted_at) = ctx.stop_on_fail_index {
            traces.push(StepRunTrace::unrunnable(
                self::kinds::iso_now_pub(),
                self::kinds::iso_now_pub(),
                format!("skipped: prior step {halted_at} halted the run"),
            ));
            continue;
        }
        let trace = dispatch(step, &mut ctx);
        // Record empty prior_output for non-shell kinds so positional
        // alignment is preserved.
        if ctx.prior_outputs.len() <= i {
            ctx.record_output(context::PriorOutput::default());
        }
        let halts = trace.stop_on_fail
            && matches!(trace.outcome, StepOutcome::Fail);
        traces.push(trace);
        if halts {
            ctx.stop_on_fail_index = Some(i);
        }
    }
    let ended_at = self::kinds::iso_now_pub();

    // Phase 4: env teardown.
    env_lifecycle::teardown(&env_handle);

    // Phase 5: verdict + persist + feedback.
    let (verdict, rationale) = derive_verdict(&graph, &traces);
    let (result_iri, persisted) = trace_writer::write_result(
        &req.workdir,
        &graph,
        &env,
        &traces,
        verdict,
        rationale,
        &req.run_activity,
        &runner_agent,
        started_at,
        ended_at,
    )?;
    let emitted_feedback =
        feedback::emit_feedback_for_failures(&req.workdir, &graph, &traces, &req.run_activity);

    let step_outcomes: Vec<StepOutcomeSummary> = traces
        .iter()
        .zip(graph.steps.iter())
        .enumerate()
        .map(|(i, (t, s))| StepOutcomeSummary {
            outcome: t.outcome,
            step_id: s.id.clone(),
            trace_id: persisted
                .step_traces
                .get(i)
                .map(|t| t.id.clone())
                .unwrap_or_default(),
        })
        .collect();

    Ok(RunGraphResponse {
        result: result_iri,
        verdict,
        step_outcomes,
        emitted_feedback,
    })
}

/// Load the `VerificationGraph` from disk. Surfaces `ArtifactNotFound`
/// when the `.ttl` is missing.
fn load_graph(workdir: &Path, graph_iri: &NamedNode) -> Result<VerificationGraph, RunnerError> {
    let id = graph_iri
        .as_str()
        .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
        .unwrap_or(graph_iri.as_str());
    let dir = workdir.join(".dec").join("verify").join("graph");
    let path = dir.join(format!("{id}.ttl"));
    if !path.is_file() {
        return Err(RunnerError::ArtifactNotFound {
            kind: "VerificationGraph".into(),
            id: id.to_string(),
        });
    }
    graph_from_turtle(&path).map_err(|e| RunnerError::Internal {
        detail: format!("parsing graph turtle: {e}"),
    })
}

fn load_env(
    workdir: &Path,
    graph: &VerificationGraph,
) -> Result<VerificationEnvironment, RunnerError> {
    let env_iri = graph.environment.as_str();
    let env_id = env_iri
        .strip_prefix(IRI_DEC_ENV_PREFIX)
        .unwrap_or(env_iri);
    let dir = workdir.join(".dec").join("verify").join("env");
    let path = dir.join(format!("{env_id}.ttl"));
    if !path.is_file() {
        return Err(RunnerError::ArtifactNotFound {
            kind: "VerificationEnvironment".into(),
            id: env_id.to_string(),
        });
    }
    env_from_turtle(&path).map_err(|e| RunnerError::Internal {
        detail: format!("parsing env turtle: {e}"),
    })
}

/// Spawn `bash -c` for the env's setup script.
fn run_setup(script: &str, workdir: &Path) -> Result<(), String> {
    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(workdir)
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// FT-097 per-graph verdict derivation, applied at runtime.
fn derive_verdict(graph: &VerificationGraph, traces: &[StepRunTrace]) -> (Verdict, String) {
    use crate::core::verify::aggregate::single_graph_verdict;
    let outcomes: Vec<StepOutcome> = traces.iter().map(|t| t.outcome).collect();
    let evidence: Vec<Vec<String>> = graph
        .steps
        .iter()
        .map(|s| {
            s.provides_evidence_for
                .iter()
                .map(|n| n.as_str().to_string())
                .collect()
        })
        .collect();
    single_graph_verdict(&outcomes, &evidence)
}

/// Persist a rejected VGR with empty step traces for Phase-1 abort
/// cases (safety violation, env setup failure).
fn persist_rejected(
    workdir: &Path,
    graph: &VerificationGraph,
    env: &VerificationEnvironment,
    run_activity: &NamedNode,
    runner_agent: &NamedNode,
    rationale: String,
) -> Result<(), RunnerError> {
    let id = mint_id(workdir);
    let result_iri = format!("{IRI_DEC_VERIFY_RESULT_PREFIX}{id}");
    let ts = self::kinds::iso_now_pub();
    let result = VerificationGraphResult {
        id: result_iri.clone(),
        result_of: graph.id.as_str().to_string(),
        ran_in_environment: env.iri().as_str().to_string(),
        verdict: Verdict::Rejected,
        started_at: ts.clone(),
        ended_at: ts.clone(),
        step_traces: Vec::new(),
        evidence_for: Vec::new(),
        rationale,
        was_generated_by: run_activity.as_str().to_string(),
        was_attributed_to: runner_agent.as_str().to_string(),
        created_at: ts,
    };

    let store = load_store_from_dump(&orchestration_dump_path(workdir)).map_err(|e| {
        RunnerError::Internal {
            detail: format!("loading store: {e}"),
        }
    })?;
    let store = Arc::new(store);
    let scope = ActiveScope::load(workdir).map_err(|e| RunnerError::Internal {
        detail: format!("loading scope: {e}"),
    })?;
    let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| RunnerError::Internal {
        detail: format!("active stream iri: {e}"),
    })?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri).map_err(|e| {
        RunnerError::Internal {
            detail: format!("opening stream writer: {e}"),
        }
    })?;
    let quads = result.to_quads(verify_result_graph());
    let mutation = Mutation::insert(quads);
    writer
        .commit(mutation)
        .map_err(|source| RunnerError::ResultWriteFailed {
            source: anyhow!("{source:#}"),
        })?;
    persist_store(&store, &orchestration_dump_path(workdir)).map_err(|e| RunnerError::Internal {
        detail: format!("persisting store: {e}"),
    })?;
    let dir = workdir.join(".dec").join("verify").join("result");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{id}.ttl")), to_canonical_turtle(&result));
    Ok(())
}

fn mint_id(workdir: &Path) -> String {
    let dir = workdir.join(".dec").join("verify").join("result");
    let _ = std::fs::create_dir_all(&dir);
    let mut max: usize = 0;
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(stem) = name.strip_suffix(".ttl") else {
                continue;
            };
            let Some(num) = stem.strip_prefix("VGR-") else {
                continue;
            };
            if let Ok(n) = num.parse::<usize>() {
                if n > max {
                    max = n;
                }
            }
        }
    }
    format!("VGR-{:03}", max + 1)
}
