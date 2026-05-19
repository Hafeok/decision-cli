//! `dec implement <FT-XXX>` — implementer-role end-to-end (FT-011 + FT-013).
//!
//! This module is the harness side of the slice-1 implementer loop. It:
//!
//! 1. Validates that the active stream authorises the `ship` goal
//!    (ADR-005 / [`crate::scope::ActiveScope`]).
//! 2. Assembles a context bundle for the target feature by invoking
//!    `product context FT-XXX --depth 1` as a subprocess (ADR-009),
//!    falling back to a minimal synthetic bundle when product-cli is
//!    not on `$PATH`.
//! 3. Computes a SHA-256 over the bundle bytes.
//! 4. Mints `Session` + `Dispatch` artifacts in the orchestration store
//!    with full PROV-O lineage (ADR-004) and the `dec:inStream` tag
//!    (ADR-005) — both via [`crate::StreamWriter`] so the tagging is
//!    structural rather than incidental.
//! 5. Spawns the code-writer worker as a subprocess in one-shot mode
//!    (`code-writer run-once`), feeding it a `DispatchPayload` on stdin
//!    and parsing the `WorkerResponse` from stdout (ADR-008).
//! 6. Persists the resulting `CodeChange` into the **product-cli graph
//!    slice** at `<product-root>/.product/graph/code-changes.nq` so the
//!    cross-store PROV-O invariant (TC-013) is machine-verifiable. The
//!    file is named so it is obvious where the link crosses repositories.
//! 7. Reports the outcome on stdout.
//!
//! Slice 1 simplifications (per ADR-010):
//! - No reactive loop — `dec implement` is invoked explicitly.
//! - No model-catalog lookup — the model id is hard-coded.
//! - No multi-role chaining — the only role is `code-writer`.
//!
//! The dispatch event ("dispatch available for code-writer") is still
//! emitted through the FT-003 / FT-004 outbox + SSE plumbing so the
//! audit story is intact and TC-011 keeps passing. The worker is
//! invoked synchronously via the one-shot subprocess for predictability
//! in slice 1 (TC-008 needs deterministic timing); the daemon-mode SSE
//! path is exercised by FT-013's stand-alone tests.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::scope::ActiveScope;
use crate::vocab::{
    orchestration_graph, IRI_DEC_DISPATCH, IRI_DEC_GRAPH_ORCHESTRATION, IRI_DEC_SESSION,
};
use crate::StreamWriter;

/// Goal verb the implementer role always pursues (ADR-005 / §3.4).
pub const IMPLEMENT_GOAL: &str = "ship";

/// Hard-coded slice-1 model id (model catalog deferred per §6.2).
pub const SLICE1_MODEL_ID: &str = "claude-sonnet-4-5";

/// Hard-coded role id (single role in slice 1).
pub const IMPLEMENTER_ROLE: &str = "code-writer";

/// IRI namespace for minted Session artifacts.
const SESSION_PREFIX: &str = "https://decision-cli.dev/ns/session/";
/// IRI namespace for minted Dispatch artifacts.
const DISPATCH_PREFIX: &str = "https://decision-cli.dev/ns/dispatch/";

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
const PROV_USED: &str = "http://www.w3.org/ns/prov#used";
const PROV_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";
const PROV_ENDED_AT_TIME: &str = "http://www.w3.org/ns/prov#endedAtTime";

const DEC_NS: &str = "https://decision-cli.dev/ns#";
const DEC_BUNDLE_REF: &str = "https://decision-cli.dev/ns#bundleRef";
const DEC_MODEL_REF: &str = "https://decision-cli.dev/ns#modelRef";
const DEC_CONTENT_HASH: &str = "https://decision-cli.dev/ns#contentHash";
const DEC_MODEL_VERSION: &str = "https://decision-cli.dev/ns#modelVersion";
const DEC_FEATURE_ID: &str = "https://decision-cli.dev/ns#featureId";
const DEC_OUTPUT_REF: &str = "https://decision-cli.dev/ns#outputRef";
const DEC_CODE_CHANGE_CLASS: &str = "https://decision-cli.dev/ns#CodeChange";
const DEC_DISPATCH_PROP: &str = "https://decision-cli.dev/ns#dispatch";
const DEC_ROLE_PROP: &str = "https://decision-cli.dev/ns#role";
const DEC_STATUS_PROP: &str = "https://decision-cli.dev/ns#status";
const DEC_FILE_PATH_PROP: &str = "https://decision-cli.dev/ns#filePath";
const DEC_FILE_SUMMARY_PROP: &str = "https://decision-cli.dev/ns#fileSummary";
const DEC_CHANGED_FILE: &str = "https://decision-cli.dev/ns#changedFile";

/// CLI arguments accepted by [`run`].
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct ImplementArgs {
    /// Feature id (e.g. `"FT-007"`).
    pub feature_id: String,
    /// Optional override of the workspace root the worker writes into.
    /// Defaults to `<workdir>/.dec/workspace/<feature-id>`.
    pub workspace: Option<PathBuf>,
    /// Optional override of the code-writer binary / module to invoke.
    /// Defaults to whatever `code-writer` resolves to on `$PATH`; when
    /// that resolution fails the harness falls back to
    /// `python3 -m code_writer.main`.
    pub worker_command: Option<String>,
    /// Optional override of product-cli's root dir. When ``None`` the
    /// harness walks up from `workdir` looking for `.product/`; when
    /// none is found, the harness writes the CodeChange artifact into
    /// `<workdir>/.product/graph/code-changes.nq` (creating the
    /// directory if necessary).
    pub product_root: Option<PathBuf>,
    /// Bundle-assembly depth passed to `product context`. Slice 1
    /// default is 1 (the same depth `product implement` uses).
    pub bundle_depth: usize,
}

impl ImplementArgs {
    /// Build with sensible defaults.
    #[must_use]
    pub fn new(feature_id: impl Into<String>) -> Self {
        Self {
            feature_id: feature_id.into(),
            workspace: None,
            worker_command: None,
            product_root: None,
            bundle_depth: 1,
        }
    }
}

/// Final result of a successful [`run`] — surfaces enough state for the
/// CLI to print a useful summary and tests to assert on outputs.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct ImplementOutcome {
    pub session_iri: String,
    pub dispatch_iri: String,
    pub code_change_iri: String,
    pub bundle_hash: String,
    pub workspace_dir: PathBuf,
    pub product_codechange_path: PathBuf,
    pub files_written: Vec<PathBuf>,
    pub worker_status: String,
    pub turn_count: u64,
    pub latency_seconds: f64,
    /// FT-017 finalisation: commit + status transition. `None` when the
    /// run errored before finalisation could run.
    pub finalize: Option<crate::finalize::FinalizeOutcome>,
}

/// Run the implementer dispatch end-to-end.
///
/// The function is intentionally synchronous: slice-1 dispatches are
/// driven by an explicit human invocation and the worker is invoked as
/// a one-shot subprocess (see module docstring). Async wiring lives in
/// `oxi-events` and is exercised by TC-011 separately.
pub fn run(workdir: &Path, args: &ImplementArgs) -> Result<ImplementOutcome> {
    // -- 1. Scope enforcement: `ship` must be authorised. ----------------
    let scope =
        ActiveScope::load(workdir).map_err(|e| anyhow!("loading active scope: {e}"))?;
    scope
        .validate_goal(IMPLEMENT_GOAL)
        .map_err(|e| anyhow!("goal refused: {e}"))?;

    // -- 2. Bundle assembly via product-cli subprocess. ------------------
    let product_root = resolve_product_root(workdir, args.product_root.as_deref());
    let bundle_markdown = assemble_bundle(&product_root, &args.feature_id, args.bundle_depth)?;
    let bundle_hash = sha256_hex(bundle_markdown.as_bytes());
    let bundle_iri = format!("urn:dec:bundle:{}:{}", args.feature_id, &bundle_hash[..16]);

    // -- 3. Mint Session + Dispatch via StreamWriter (FT-010 / ADR-005).
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    let store = Arc::new(Store::new().context("opening in-memory orchestration store")?);
    let bytes = fs::read(&dump_path)
        .with_context(|| format!("reading {}", dump_path.display()))?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump_path.display()))?;

    let stream_iri = NamedNode::new(&scope.stream_iri)
        .with_context(|| format!("active stream IRI {}", scope.stream_iri))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri.clone())
        .context("binding StreamWriter to active stream")?;

    let session_uuid = uuid::Uuid::new_v4();
    let dispatch_uuid = uuid::Uuid::new_v4();
    let session_iri = NamedNode::new(format!("{SESSION_PREFIX}{session_uuid}"))
        .context("minting session IRI")?;
    let dispatch_iri = NamedNode::new(format!("{DISPATCH_PREFIX}{dispatch_uuid}"))
        .context("minting dispatch IRI")?;
    let bundle_ref = NamedNode::new(&bundle_iri).context("minting bundle ref IRI")?;
    let model_ref =
        NamedNode::new(format!("urn:dec:model:{SLICE1_MODEL_ID}")).context("minting model ref")?;

    let started_at = Utc::now().to_rfc3339();
    let session_quads = build_session_quads(
        &session_iri,
        &dispatch_iri,
        &bundle_ref,
        &bundle_hash,
        &model_ref,
        SLICE1_MODEL_ID,
        &args.feature_id,
        &started_at,
    );
    let dispatch_quads = build_dispatch_quads(&dispatch_iri, &session_iri, &started_at);

    let mut mint = Mutation::insert(session_quads.iter().cloned());
    for q in &dispatch_quads {
        mint.inserts.push(q.clone());
    }
    mint = mint.with_cause(format!("dec implement {}", args.feature_id));
    writer
        .commit(mint)
        .context("committing Session + Dispatch artifacts")?;

    // -- 4. Spawn the code-writer worker (one-shot mode). ---------------
    let workspace_dir = match args.workspace.clone() {
        Some(p) => p,
        None => workdir
            .join(".dec")
            .join("workspace")
            .join(&args.feature_id),
    };
    fs::create_dir_all(&workspace_dir)
        .with_context(|| format!("preparing workspace {}", workspace_dir.display()))?;

    // FT-011: max_turns and allowed_tools are omitted in slice 1. The
    // worker invokes `claude -p --dangerously-skip-permissions` with no
    // turn cap; timeout_seconds is the sole upper bound on runtime.
    let dispatch_payload = DispatchPayloadJson {
        dispatch_id: dispatch_iri.as_str().to_string(),
        session_id: session_iri.as_str().to_string(),
        feature_id: args.feature_id.clone(),
        bundle_markdown,
        bundle_hash: bundle_hash.clone(),
        workspace_path: workspace_dir.canonicalize().unwrap_or(workspace_dir.clone()).to_string_lossy().into_owned(),
        model_id: SLICE1_MODEL_ID.to_string(),
        timeout_seconds: 1800,
    };
    let response = run_worker(args.worker_command.as_deref(), &dispatch_payload)
        .context("running code-writer worker")?;

    if response.status != "ok" {
        let detail = response
            .error
            .as_ref()
            .map(|e| {
                if e.detail.is_empty() {
                    format!("{}: {}", e.category, e.message)
                } else {
                    format!("{}: {}\n--- worker detail ---\n{}", e.category, e.message, e.detail)
                }
            })
            .unwrap_or_else(|| "(no error detail)".into());
        // Persist the failure on the Session and bail. Slice 1 still
        // records the Session so audits never see a silent failure.
        let mut fail = Mutation::default();
        let g: GraphName = orchestration_graph().into_owned().into();
        fail.inserts.push(Quad::new(
            session_iri.clone(),
            NamedNodeRef::new_unchecked(DEC_STATUS_PROP).into_owned(),
            Literal::new_simple_literal(format!("failed: {detail}")),
            g,
        ));
        writer
            .commit(fail.with_cause("dec implement: worker failure"))
            .ok();
        return Err(anyhow!("code-writer worker reported failure: {detail}"));
    }

    let code_change = response
        .code_change
        .as_ref()
        .ok_or_else(|| anyhow!("worker reported status=ok with no code_change"))?;

    // -- 5. Record CodeChange in product-cli graph slice. ---------------
    let codechange_path = product_codechange_path(&product_root);
    write_codechange_to_product_graph(
        &codechange_path,
        code_change,
        &session_iri,
        &dispatch_iri,
        &args.feature_id,
    )
    .with_context(|| {
        format!(
            "writing CodeChange artifact to product graph at {}",
            codechange_path.display()
        )
    })?;

    // -- 6. Mark the Session complete with output ref + telemetry. ------
    let completed_at = Utc::now().to_rfc3339();
    let mut complete = Mutation::default();
    let g: GraphName = orchestration_graph().into_owned().into();
    let code_change_iri = NamedNode::new(&code_change.iri)
        .with_context(|| format!("code change IRI {}", code_change.iri))?;
    complete.inserts.push(Quad::new(
        session_iri.clone(),
        NamedNodeRef::new_unchecked(DEC_OUTPUT_REF).into_owned(),
        code_change_iri.clone(),
        g.clone(),
    ));
    complete.inserts.push(Quad::new(
        session_iri.clone(),
        NamedNodeRef::new_unchecked(DEC_STATUS_PROP).into_owned(),
        Literal::new_simple_literal("complete"),
        g.clone(),
    ));
    complete.inserts.push(Quad::new(
        session_iri.clone(),
        NamedNodeRef::new_unchecked(PROV_ENDED_AT_TIME).into_owned(),
        Literal::new_simple_literal(&completed_at),
        g.clone(),
    ));
    writer
        .commit(complete.with_cause("dec implement: worker complete"))
        .context("committing session completion")?;

    // -- 7. Persist the updated store. ----------------------------------
    persist_store(&store, &dump_path)?;

    // -- 8. FT-017 finalisation: commit + feature status transition. ----
    // Runs only after the orchestration record is durable on disk so a
    // commit/status failure surfaces as a finalisation error without
    // invalidating the Session record.
    let finalize_input = crate::finalize::FinalizeInput {
        repo_root: workdir,
        product_root: &product_root,
        feature_id: &args.feature_id,
        session_iri: session_iri.as_str(),
        dispatch_iri: dispatch_iri.as_str(),
        code_change_iri: code_change.iri.as_str(),
        bundle_hash: &bundle_hash,
        worker_summary: &code_change.summary,
    };
    let finalize_outcome = crate::finalize::finalize_run(&finalize_input)
        .context("finalising dec implement run (FT-017)")?;

    let files_written: Vec<PathBuf> = code_change
        .files
        .iter()
        .map(|f| workspace_dir.join(&f.path))
        .collect();
    Ok(ImplementOutcome {
        session_iri: session_iri.as_str().to_string(),
        dispatch_iri: dispatch_iri.as_str().to_string(),
        code_change_iri: code_change.iri.clone(),
        bundle_hash,
        workspace_dir,
        product_codechange_path: codechange_path,
        files_written,
        worker_status: response.status.clone(),
        turn_count: response.telemetry.turn_count,
        latency_seconds: response.telemetry.latency_seconds,
        finalize: Some(finalize_outcome),
    })
}

/// `dec session show <iri>` payload — load Session triples and render.
pub fn session_show(workdir: &Path, session_iri: &str) -> Result<String> {
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump_path)
        .with_context(|| format!("reading {}", dump_path.display()))?;
    let store = Store::new().context("opening session-show store")?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump_path.display()))?;

    // The contentHash + modelVersion are reachable via the bundle/model
    // refs (prov:used). The audit query hops through both refs so the
    // shape matches TC-012's invariant directly. Session triples live
    // in the orchestration named graph; the query wraps each pattern in
    // a `GRAPH ?g` block to match regardless of which graph an artifact
    // happens to sit in.
    let q = format!(
        "PREFIX dec: <{DEC_NS}>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?bundleHash ?modelVersion ?feature ?status ?output ?stream WHERE {{
  GRAPH ?g {{
    <{session_iri}> prov:used ?bundle ;
                    prov:used ?model ;
                    dec:featureId ?feature ;
                    dec:inStream ?stream .
    ?bundle dec:contentHash ?bundleHash .
    ?model dec:modelVersion ?modelVersion .
    OPTIONAL {{ <{session_iri}> dec:status ?status }}
    OPTIONAL {{ <{session_iri}> dec:outputRef ?output }}
  }}
}} LIMIT 1",
    );

    let results = store.query(q.as_str()).context("session-show SPARQL")?;
    let oxigraph::sparql::QueryResults::Solutions(mut sols) = results else {
        return Err(anyhow!("session-show: unexpected SPARQL result shape"));
    };
    let Some(sol_res) = sols.next() else {
        return Err(anyhow!("session-show: no Session with IRI <{session_iri}>"));
    };
    let sol = sol_res.context("session-show SPARQL solution")?;
    let bh = sol
        .get("bundleHash")
        .map(term_literal)
        .unwrap_or_else(|| "(unknown)".into());
    let mv = sol
        .get("modelVersion")
        .map(term_literal)
        .unwrap_or_else(|| "(unknown)".into());
    let ft = sol
        .get("feature")
        .map(term_literal)
        .unwrap_or_else(|| "(unknown)".into());
    let st = sol
        .get("status")
        .map(term_literal)
        .unwrap_or_else(|| "(pending)".into());
    let ou = sol
        .get("output")
        .map(term_iri)
        .unwrap_or_else(|| "(none)".into());
    let sr = sol
        .get("stream")
        .map(term_iri)
        .unwrap_or_else(|| "(unknown)".into());

    Ok(format!(
        "Session: {session_iri}\n  Feature:        {ft}\n  Bundle hash:    {bh}\n  Model version:  {mv}\n  Stream:         {sr}\n  Status:         {st}\n  Output:         {ou}\n",
    ))
}

fn term_literal(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::Literal(lit) => lit.value().to_string(),
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

fn term_iri(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_session_quads(
    session: &NamedNode,
    dispatch: &NamedNode,
    bundle: &NamedNode,
    bundle_hash: &str,
    model: &NamedNode,
    model_version: &str,
    feature_id: &str,
    started_at: &str,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let session_class = NamedNodeRef::new_unchecked(IRI_DEC_SESSION).into_owned();
    let activity = NamedNodeRef::new_unchecked(PROV_ACTIVITY).into_owned();
    let used = NamedNodeRef::new_unchecked(PROV_USED).into_owned();
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME).into_owned();
    let content_hash = NamedNodeRef::new_unchecked(DEC_CONTENT_HASH).into_owned();
    let model_version_pred = NamedNodeRef::new_unchecked(DEC_MODEL_VERSION).into_owned();
    let feature_pred = NamedNodeRef::new_unchecked(DEC_FEATURE_ID).into_owned();
    let bundle_ref_pred = NamedNodeRef::new_unchecked(DEC_BUNDLE_REF).into_owned();
    let model_ref_pred = NamedNodeRef::new_unchecked(DEC_MODEL_REF).into_owned();
    let dispatch_pred = NamedNodeRef::new_unchecked(DEC_DISPATCH_PROP).into_owned();
    let role_pred = NamedNodeRef::new_unchecked(DEC_ROLE_PROP).into_owned();
    vec![
        Quad::new(
            session.clone(),
            rdf_type.clone(),
            session_class,
            g.clone(),
        ),
        Quad::new(session.clone(), rdf_type.clone(), activity, g.clone()),
        Quad::new(session.clone(), used.clone(), bundle.clone(), g.clone()),
        Quad::new(session.clone(), used.clone(), model.clone(), g.clone()),
        Quad::new(
            bundle.clone(),
            content_hash,
            Literal::new_simple_literal(bundle_hash),
            g.clone(),
        ),
        Quad::new(
            model.clone(),
            model_version_pred,
            Literal::new_simple_literal(model_version),
            g.clone(),
        ),
        Quad::new(
            session.clone(),
            feature_pred,
            Literal::new_simple_literal(feature_id),
            g.clone(),
        ),
        Quad::new(
            session.clone(),
            bundle_ref_pred,
            bundle.clone(),
            g.clone(),
        ),
        Quad::new(session.clone(), model_ref_pred, model.clone(), g.clone()),
        Quad::new(
            session.clone(),
            at_time,
            Literal::new_simple_literal(started_at),
            g.clone(),
        ),
        Quad::new(
            session.clone(),
            dispatch_pred,
            dispatch.clone(),
            g.clone(),
        ),
        Quad::new(
            session.clone(),
            role_pred,
            Literal::new_simple_literal(IMPLEMENTER_ROLE),
            g,
        ),
    ]
}

fn build_dispatch_quads(
    dispatch: &NamedNode,
    session: &NamedNode,
    started_at: &str,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let dispatch_class = NamedNodeRef::new_unchecked(IRI_DEC_DISPATCH).into_owned();
    let generated = NamedNodeRef::new_unchecked(PROV_GENERATED_BY).into_owned();
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME).into_owned();
    let role_pred = NamedNodeRef::new_unchecked(DEC_ROLE_PROP).into_owned();
    vec![
        Quad::new(dispatch.clone(), rdf_type, dispatch_class, g.clone()),
        Quad::new(
            dispatch.clone(),
            generated,
            session.clone(),
            g.clone(),
        ),
        Quad::new(
            dispatch.clone(),
            role_pred,
            Literal::new_simple_literal(IMPLEMENTER_ROLE),
            g.clone(),
        ),
        Quad::new(
            dispatch.clone(),
            at_time,
            Literal::new_simple_literal(started_at),
            g,
        ),
    ]
}

/// Resolve a product-cli root by walking up from `workdir`. When no
/// `.product/` is found we default to `workdir/.product/`. Tests and
/// CI fixtures may also pass `--product-root` explicitly.
pub fn resolve_product_root(workdir: &Path, override_path: Option<&Path>) -> PathBuf {
    if let Some(p) = override_path {
        return p.to_path_buf();
    }
    let mut current = Some(workdir.to_path_buf());
    while let Some(p) = current {
        if p.join(".product").is_dir() {
            return p;
        }
        current = p.parent().map(Path::to_path_buf);
    }
    workdir.to_path_buf()
}

/// The fixed location for the product-cli CodeChange graph slice.
fn product_codechange_path(product_root: &Path) -> PathBuf {
    product_root
        .join(".product")
        .join("graph")
        .join("code-changes.nq")
}

fn assemble_bundle(product_root: &Path, feature_id: &str, depth: usize) -> Result<String> {
    // Best-effort: `product context FT-XXX --depth N --root <product_root>`.
    // When `product` is not on PATH we fall back to a minimal synthetic
    // bundle so the harness still proves the flow end-to-end. The
    // bundle content is part of the SHA-256 either way, so tests can
    // pin the hash for determinism by setting the override.
    let mut cmd = Command::new("product");
    cmd.arg("context")
        .arg(feature_id)
        .arg("--depth")
        .arg(depth.to_string())
        .arg("--root")
        .arg(product_root);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    match cmd.output() {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout).into_owned();
            if body.trim().is_empty() {
                Ok(synthetic_bundle(feature_id))
            } else {
                Ok(body)
            }
        }
        Ok(out) => {
            // product-cli reported an error — fall through to the
            // synthetic bundle so slice-1 testing remains hermetic.
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            tracing::warn!(
                target = "dec.implement",
                feature = feature_id,
                "product-cli context failed: {stderr}; using synthetic bundle"
            );
            Ok(synthetic_bundle(feature_id))
        }
        Err(_) => Ok(synthetic_bundle(feature_id)),
    }
}

fn synthetic_bundle(feature_id: &str) -> String {
    format!(
        "# {feature_id} — synthetic context bundle\n\n\
This is a slice-1 fallback bundle produced because `product context` is\n\
not available in the current environment. The harness still computes a\n\
stable SHA-256 over these bytes so the Session's `dec:contentHash` is\n\
deterministic.\n",
    )
}

fn persist_store(store: &Store, dump_path: &Path) -> Result<()> {
    if let Some(parent) = dump_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp = dump_path.with_extension("nq.tmp");
    let mut buf: Vec<u8> = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .context("dumping orchestration store")?;
    fs::write(&tmp, &buf).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, dump_path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), dump_path.display()))?;
    Ok(())
}

// -- Worker payload <-> response ------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct DispatchPayloadJson {
    dispatch_id: String,
    session_id: String,
    feature_id: String,
    bundle_markdown: String,
    bundle_hash: String,
    workspace_path: String,
    model_id: String,
    timeout_seconds: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkerResponseJson {
    #[allow(dead_code)]
    dispatch_id: String,
    #[allow(dead_code)]
    session_id: String,
    status: String,
    code_change: Option<CodeChangeJson>,
    #[serde(default)]
    telemetry: TelemetryJson,
    #[serde(default)]
    error: Option<ErrorJson>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodeChangeJson {
    iri: String,
    #[allow(dead_code)]
    feature_id: String,
    #[allow(dead_code)]
    session_id: String,
    #[serde(default)]
    files: Vec<FileWriteJson>,
    #[serde(default)]
    summary: String,
}

#[derive(Debug, Clone, Deserialize)]
struct FileWriteJson {
    path: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    bytes_written: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TelemetryJson {
    #[serde(default)]
    turn_count: u64,
    #[serde(default)]
    latency_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct ErrorJson {
    category: String,
    message: String,
    #[serde(default)]
    detail: String,
}

fn run_worker(
    worker_command: Option<&str>,
    payload: &DispatchPayloadJson,
) -> Result<WorkerResponseJson> {
    // Pick the invocation: explicit override > env override > `code-writer` on PATH >
    // python3 module fallback.
    let mut cmd = if let Some(custom) = worker_command {
        build_shell_command(custom)
    } else if let Ok(env_cmd) = std::env::var("CODE_WRITER_CMD") {
        build_shell_command(&env_cmd)
    } else if which("code-writer").is_some() {
        let mut c = Command::new("code-writer");
        c.arg("run-once");
        c
    } else {
        // Fall back to invoking the module via python3. The python3
        // entry point pulls in the same code paths as the installed
        // console script.
        let mut c = Command::new("python3");
        c.arg("-m").arg("code_writer.main").arg("run-once");
        c
    };

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .context("spawning code-writer worker subprocess")?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("worker stdin closed"))?;
        let body = serde_json::to_vec(payload).context("serialising DispatchPayload")?;
        stdin
            .write_all(&body)
            .context("writing DispatchPayload to worker stdin")?;
    }
    let output = child
        .wait_with_output()
        .context("waiting for worker subprocess")?;
    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(anyhow!(
            "code-writer worker exited with {} and no stdout. stderr: {stderr}",
            output.status
        ));
    }
    // The worker writes one JSON response on stdout. Split at the last
    // newline so we tolerate diagnostics appended before the payload.
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| anyhow!("worker produced no parseable stdout"))?;
    let response: WorkerResponseJson = serde_json::from_str(line)
        .with_context(|| format!("parsing worker response: {line}"))?;
    Ok(response)
}

fn build_shell_command(custom: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(format!("{custom} run-once"));
    c
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn write_codechange_to_product_graph(
    path: &Path,
    code_change: &CodeChangeJson,
    session_iri: &NamedNode,
    dispatch_iri: &NamedNode,
    feature_id: &str,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    // We use a stable named graph identifier for the product-cli code-change slice.
    let product_graph_iri = "https://product-meta/graph/code-changes";
    let g = NamedNode::new(product_graph_iri).context("product graph IRI")?;
    let store = Store::new().context("opening product codechange store")?;
    if path.exists() {
        let bytes = fs::read(path)
            .with_context(|| format!("reading {}", path.display()))?;
        if !bytes.is_empty() {
            store
                .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
                .with_context(|| format!("loading {}", path.display()))?;
        }
    }
    let cc_iri = NamedNode::new(&code_change.iri).context("code change IRI")?;
    let class = NamedNodeRef::new_unchecked(DEC_CODE_CHANGE_CLASS).into_owned();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let prov_generated = NamedNodeRef::new_unchecked(PROV_GENERATED_BY).into_owned();
    let dispatch_pred = NamedNodeRef::new_unchecked(DEC_DISPATCH_PROP).into_owned();
    let feature_pred = NamedNodeRef::new_unchecked(DEC_FEATURE_ID).into_owned();
    let file_pred = NamedNodeRef::new_unchecked(DEC_CHANGED_FILE).into_owned();
    let path_pred = NamedNodeRef::new_unchecked(DEC_FILE_PATH_PROP).into_owned();
    let summary_pred = NamedNodeRef::new_unchecked(DEC_FILE_SUMMARY_PROP).into_owned();
    let graph = GraphName::NamedNode(g.clone());
    let mut quads: Vec<Quad> = vec![
        Quad::new(cc_iri.clone(), rdf_type, class, graph.clone()),
        Quad::new(
            cc_iri.clone(),
            prov_generated,
            session_iri.clone(),
            graph.clone(),
        ),
        Quad::new(
            cc_iri.clone(),
            dispatch_pred,
            dispatch_iri.clone(),
            graph.clone(),
        ),
        Quad::new(
            cc_iri.clone(),
            feature_pred,
            Literal::new_simple_literal(feature_id),
            graph.clone(),
        ),
        Quad::new(
            cc_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#summary").into_owned(),
            Literal::new_simple_literal(&code_change.summary),
            graph.clone(),
        ),
    ];
    for file in &code_change.files {
        let file_node = NamedNode::new(format!("{}/file/{}", code_change.iri, sanitize_iri_tail(&file.path)))
            .with_context(|| format!("minting file IRI for {}", file.path))?;
        quads.push(Quad::new(
            cc_iri.clone(),
            file_pred.clone(),
            file_node.clone(),
            graph.clone(),
        ));
        quads.push(Quad::new(
            file_node.clone(),
            path_pred.clone(),
            Literal::new_simple_literal(&file.path),
            graph.clone(),
        ));
        if !file.summary.is_empty() {
            quads.push(Quad::new(
                file_node.clone(),
                summary_pred.clone(),
                Literal::new_simple_literal(&file.summary),
                graph.clone(),
            ));
        }
        if file.bytes_written > 0 {
            quads.push(Quad::new(
                file_node,
                NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#bytesWritten").into_owned(),
                Literal::new_simple_literal(file.bytes_written.to_string()),
                graph.clone(),
            ));
        }
    }
    store
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .with_context(|| format!("inserting CodeChange triples into {}", path.display()))?;
    let mut buf: Vec<u8> = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .context("dumping product CodeChange store")?;
    let tmp = path.with_extension("nq.tmp");
    fs::write(&tmp, &buf).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
    // Silence unused warning when the store happens to be empty after insert.
    let _ = IRI_DEC_GRAPH_ORCHESTRATION;
    Ok(())
}

fn sanitize_iri_tail(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '/' | '\\' | ' ' | '#' | '?' | '%' | '&' => out.push('_'),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' => out.push(c),
            _ => out.push('_'),
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}
