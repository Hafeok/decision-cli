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
mod lifecycle;
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

use bundle::{assemble_bundle, persist_store, sha256_hex};
use lifecycle::{
    assemble_implement_outcome, build_dispatch_payload, commit_session_completion,
    extract_code_change, finalize_implement_run, persist_code_change,
};
use quads::{build_dispatch_quads, build_failure_quad, build_session_quads};
use vocab::{DISPATCH_PREFIX, SESSION_PREFIX};
use worker::{format_worker_failure, run_worker};

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
    validate_scope(workdir)?;
    let bundle = prepare_bundle(workdir, args)?;
    let (store, writer, dump_path) = load_store_and_writer(workdir)?;
    let iris = mint_dispatch_iris(args, &bundle)?;
    commit_initial_session(&writer, &store, &dump_path, args, &bundle, &iris)?;
    let workspace_dir = resolve_workspace_dir(workdir, args)?;
    Ok(DispatchContext {
        session_iri: iris.session,
        dispatch_iri: iris.dispatch,
        bundle_hash: bundle.hash,
        bundle_markdown: bundle.markdown,
        workspace_dir,
        product_root: bundle.product_root,
        dump_path,
        store,
        writer,
    })
}

/// Load the active scope and reject the dispatch if the stream does not
/// authorise the implementer goal ([`IMPLEMENT_GOAL`]).
fn validate_scope(workdir: &Path) -> Result<()> {
    let scope = ActiveScope::load(workdir).map_err(|e| anyhow!("loading active scope: {e}"))?;
    scope
        .validate_goal(IMPLEMENT_GOAL)
        .map_err(|e| anyhow!("goal refused: {e}"))
}

struct PreparedBundle {
    markdown: String,
    hash: String,
    iri: String,
    product_root: PathBuf,
}

/// Assemble the context bundle via `product context` and compute its
/// content hash + the bundle artifact IRI consumed downstream.
fn prepare_bundle(workdir: &Path, args: &ImplementArgs) -> Result<PreparedBundle> {
    let product_root = resolve_product_root(workdir, args.product_root.as_deref());
    let markdown = assemble_bundle(&product_root, &args.feature_id, args.bundle_depth)?;
    let hash = sha256_hex(markdown.as_bytes());
    let iri = format!("urn:dec:bundle:{}:{}", args.feature_id, &hash[..16]);
    Ok(PreparedBundle {
        markdown,
        hash,
        iri,
        product_root,
    })
}

/// Open the persisted orchestration store and bind a [`StreamWriter`] to
/// the active value-stream identity.
fn load_store_and_writer(workdir: &Path) -> Result<(Arc<Store>, StreamWriter, PathBuf)> {
    let scope = ActiveScope::load(workdir).map_err(|e| anyhow!("loading active scope: {e}"))?;
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
    Ok((store, writer, dump_path))
}

struct MintedIris {
    session: NamedNode,
    dispatch: NamedNode,
    bundle_ref: NamedNode,
    model_ref: NamedNode,
}

/// Mint the per-dispatch session / dispatch / bundle / model IRIs.
fn mint_dispatch_iris(_args: &ImplementArgs, bundle: &PreparedBundle) -> Result<MintedIris> {
    let session_uuid = uuid::Uuid::new_v4();
    let dispatch_uuid = uuid::Uuid::new_v4();
    Ok(MintedIris {
        session: NamedNode::new(format!("{SESSION_PREFIX}{session_uuid}"))
            .context("minting session IRI")?,
        dispatch: NamedNode::new(format!("{DISPATCH_PREFIX}{dispatch_uuid}"))
            .context("minting dispatch IRI")?,
        bundle_ref: NamedNode::new(&bundle.iri).context("minting bundle ref IRI")?,
        model_ref: NamedNode::new(format!("urn:dec:model:{SLICE1_MODEL_ID}"))
            .context("minting model ref")?,
    })
}

/// Commit the `Session + Dispatch` quad set via [`StreamWriter`] and
/// persist the store snapshot used by downstream commands.
fn commit_initial_session(
    writer: &StreamWriter,
    store: &Store,
    dump_path: &Path,
    args: &ImplementArgs,
    bundle: &PreparedBundle,
    iris: &MintedIris,
) -> Result<()> {
    let started_at = Utc::now().to_rfc3339();
    let session_quads = build_session_quads(
        &iris.session,
        &iris.dispatch,
        &iris.bundle_ref,
        &bundle.hash,
        &iris.model_ref,
        SLICE1_MODEL_ID,
        &args.feature_id,
        &started_at,
    );
    let dispatch_quads = build_dispatch_quads(&iris.dispatch, &iris.session, &started_at);
    let mut mint = Mutation::insert(session_quads.iter().cloned());
    for q in &dispatch_quads {
        mint.inserts.push(q.clone());
    }
    mint = mint.with_cause(format!("dec implement {}", args.feature_id));
    writer
        .commit(mint)
        .context("committing Session + Dispatch artifacts")?;
    persist_store(store, dump_path)
}

/// Pick the workspace directory the worker will be confined to,
/// defaulting to `.dec/workspace/<feature_id>/`.
fn resolve_workspace_dir(workdir: &Path, args: &ImplementArgs) -> Result<PathBuf> {
    let workspace_dir = args.workspace.clone().unwrap_or_else(|| {
        workdir
            .join(".dec")
            .join("workspace")
            .join(&args.feature_id)
    });
    fs::create_dir_all(&workspace_dir)
        .with_context(|| format!("preparing workspace {}", workspace_dir.display()))?;
    Ok(workspace_dir)
}

fn record_worker_failure(ctx: &DispatchContext, detail: &str) {
    let mut fail = Mutation::default();
    fail.inserts
        .push(build_failure_quad(&ctx.session_iri, detail));
    ctx.writer
        .commit(fail.with_cause("dec implement: worker failure"))
        .ok();
    let _ = persist_store(&ctx.store, &ctx.dump_path);
}

/// Run the implementer dispatch end-to-end. See module docs.
pub fn run(workdir: &Path, args: &ImplementArgs) -> Result<ImplementOutcome> {
    let ctx = prepare_dispatch(workdir, args)?;
    let dispatch_payload = build_dispatch_payload(&ctx, args);
    let response = run_worker(args.worker_command.as_deref(), &dispatch_payload)
        .context("running code-writer worker")?;
    if response.status != "ok" {
        let detail = format_worker_failure(response.error.as_ref());
        record_worker_failure(&ctx, &detail);
        return Err(anyhow!("code-writer worker reported failure: {detail}"));
    }
    let code_change = extract_code_change(&response)?;
    let codechange_path = persist_code_change(&ctx, args, code_change)?;
    commit_session_completion(&ctx, code_change)?;
    let finalize_outcome = finalize_implement_run(workdir, &ctx, args, code_change)?;
    Ok(assemble_implement_outcome(
        ctx,
        &response,
        code_change,
        codechange_path,
        finalize_outcome,
    ))
}
