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
//! 6. Persists the resulting `CodeChange` into the product-cli graph
//!    slice at `<product-root>/.product/graph/code-changes.nq` so the
//!    cross-store PROV-O invariant (TC-013) is machine-verifiable.
//! 7. Reports the outcome on stdout.

mod bundle;
mod codechange;
mod quads;
mod session_show;
mod vocab;
mod worker;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::io::RdfFormat;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::scope::ActiveScope;
use crate::StreamWriter;

pub use bundle::resolve_product_root;
pub use session_show::session_show;

use bundle::{assemble_bundle, persist_store, product_codechange_path, sha256_hex};
use codechange::write_codechange_to_product_graph;
use quads::{
    build_completion_quads, build_dispatch_quads, build_failure_quad, build_session_quads,
};
use vocab::{DISPATCH_PREFIX, SESSION_PREFIX};
use worker::{format_worker_failure, run_worker, DispatchPayloadJson};

/// Goal verb the implementer role always pursues (ADR-005 / §3.4).
pub const IMPLEMENT_GOAL: &str = "ship";

/// Hard-coded slice-1 model id (model catalog deferred per §6.2).
pub const SLICE1_MODEL_ID: &str = "claude-sonnet-4-5";

/// Hard-coded role id (single role in slice 1).
pub const IMPLEMENTER_ROLE: &str = "code-writer";

/// CLI arguments accepted by [`run`].
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct ImplementArgs {
    pub feature_id: String,
    pub workspace: Option<PathBuf>,
    pub worker_command: Option<String>,
    pub product_root: Option<PathBuf>,
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

/// Final result of a successful [`run`].
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

struct DispatchContext {
    session_iri: NamedNode,
    dispatch_iri: NamedNode,
    bundle_hash: String,
    bundle_markdown: String,
    workspace_dir: PathBuf,
    product_root: PathBuf,
    dump_path: PathBuf,
    store: Arc<Store>,
    writer: StreamWriter,
}

fn prepare_dispatch(workdir: &Path, args: &ImplementArgs) -> Result<DispatchContext> {
    let scope = ActiveScope::load(workdir).map_err(|e| anyhow!("loading active scope: {e}"))?;
    scope
        .validate_goal(IMPLEMENT_GOAL)
        .map_err(|e| anyhow!("goal refused: {e}"))?;

    let product_root = resolve_product_root(workdir, args.product_root.as_deref());
    let bundle_markdown = assemble_bundle(&product_root, &args.feature_id, args.bundle_depth)?;
    let bundle_hash = sha256_hex(bundle_markdown.as_bytes());
    let bundle_iri = format!("urn:dec:bundle:{}:{}", args.feature_id, &bundle_hash[..16]);

    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    let store = Arc::new(Store::new().context("opening in-memory orchestration store")?);
    let bytes = fs::read(&dump_path).with_context(|| format!("reading {}", dump_path.display()))?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump_path.display()))?;

    let stream_iri = NamedNode::new(&scope.stream_iri)
        .with_context(|| format!("active stream IRI {}", scope.stream_iri))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .context("binding StreamWriter to active stream")?;

    let session_uuid = uuid::Uuid::new_v4();
    let dispatch_uuid = uuid::Uuid::new_v4();
    let session_iri = NamedNode::new(format!("{SESSION_PREFIX}{session_uuid}"))
        .context("minting session IRI")?;
    let dispatch_iri = NamedNode::new(format!("{DISPATCH_PREFIX}{dispatch_uuid}"))
        .context("minting dispatch IRI")?;
    let bundle_ref = NamedNode::new(&bundle_iri).context("minting bundle ref IRI")?;
    let model_ref = NamedNode::new(format!("urn:dec:model:{SLICE1_MODEL_ID}"))
        .context("minting model ref")?;

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
    persist_store(&store, &dump_path)?;

    let workspace_dir = args
        .workspace
        .clone()
        .unwrap_or_else(|| workdir.join(".dec").join("workspace").join(&args.feature_id));
    fs::create_dir_all(&workspace_dir)
        .with_context(|| format!("preparing workspace {}", workspace_dir.display()))?;

    Ok(DispatchContext {
        session_iri,
        dispatch_iri,
        bundle_hash,
        bundle_markdown,
        workspace_dir,
        product_root,
        dump_path,
        store,
        writer,
    })
}

fn record_worker_failure(ctx: &DispatchContext, detail: &str) {
    let mut fail = Mutation::default();
    fail.inserts.push(build_failure_quad(&ctx.session_iri, detail));
    ctx.writer
        .commit(fail.with_cause("dec implement: worker failure"))
        .ok();
    let _ = persist_store(&ctx.store, &ctx.dump_path);
}

/// Run the implementer dispatch end-to-end. See module docs.
pub fn run(workdir: &Path, args: &ImplementArgs) -> Result<ImplementOutcome> {
    let ctx = prepare_dispatch(workdir, args)?;

    let dispatch_payload = DispatchPayloadJson {
        dispatch_id: ctx.dispatch_iri.as_str().to_string(),
        session_id: ctx.session_iri.as_str().to_string(),
        feature_id: args.feature_id.clone(),
        bundle_markdown: ctx.bundle_markdown.clone(),
        bundle_hash: ctx.bundle_hash.clone(),
        workspace_path: ctx
            .workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| ctx.workspace_dir.clone())
            .to_string_lossy()
            .into_owned(),
        model_id: SLICE1_MODEL_ID.to_string(),
        timeout_seconds: 1800,
    };
    let response = run_worker(args.worker_command.as_deref(), &dispatch_payload)
        .context("running code-writer worker")?;

    if response.status != "ok" {
        let detail = format_worker_failure(response.error.as_ref());
        record_worker_failure(&ctx, &detail);
        return Err(anyhow!("code-writer worker reported failure: {detail}"));
    }

    let code_change = response
        .code_change
        .as_ref()
        .ok_or_else(|| anyhow!("worker reported status=ok with no code_change"))?;

    let codechange_path = product_codechange_path(&ctx.product_root);
    write_codechange_to_product_graph(
        &codechange_path,
        code_change,
        &ctx.session_iri,
        &ctx.dispatch_iri,
        &args.feature_id,
    )
    .with_context(|| {
        format!(
            "writing CodeChange artifact to product graph at {}",
            codechange_path.display()
        )
    })?;

    let completed_at = Utc::now().to_rfc3339();
    let code_change_iri = NamedNode::new(&code_change.iri)
        .with_context(|| format!("code change IRI {}", code_change.iri))?;
    let complete_quads = build_completion_quads(&ctx.session_iri, &code_change_iri, &completed_at);
    let mut complete = Mutation::default();
    for q in complete_quads {
        complete.inserts.push(q);
    }
    ctx.writer
        .commit(complete.with_cause("dec implement: worker complete"))
        .context("committing session completion")?;
    persist_store(&ctx.store, &ctx.dump_path)?;

    let finalize_input = crate::finalize::FinalizeInput {
        repo_root: workdir,
        product_root: &ctx.product_root,
        feature_id: &args.feature_id,
        session_iri: ctx.session_iri.as_str(),
        dispatch_iri: ctx.dispatch_iri.as_str(),
        code_change_iri: code_change.iri.as_str(),
        bundle_hash: &ctx.bundle_hash,
        worker_summary: &code_change.summary,
    };
    let finalize_outcome = crate::finalize::finalize_run(&finalize_input)
        .context("finalising dec implement run (FT-017)")?;

    let files_written: Vec<PathBuf> = code_change
        .files
        .iter()
        .map(|f| ctx.workspace_dir.join(&f.path))
        .collect();
    Ok(ImplementOutcome {
        session_iri: ctx.session_iri.as_str().to_string(),
        dispatch_iri: ctx.dispatch_iri.as_str().to_string(),
        code_change_iri: code_change.iri.clone(),
        bundle_hash: ctx.bundle_hash,
        workspace_dir: ctx.workspace_dir,
        product_codechange_path: codechange_path,
        files_written,
        worker_status: response.status.clone(),
        turn_count: response.telemetry.turn_count,
        latency_seconds: response.telemetry.latency_seconds,
        finalize: Some(finalize_outcome),
    })
}
