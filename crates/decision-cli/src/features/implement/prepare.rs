//! Setup helpers for the implementer dispatch (FT-011 / FT-021).
//!
//! Split out of `mod.rs` so the run pipeline stays under ADR-013 Rule 1's
//! 400-line hard cap. Owns:
//!
//!   * worker preflight + bundle assembly,
//!   * store / writer open,
//!   * IRI minting + the initial Session/Dispatch quad commit,
//!   * minting the paired `dec:DispatchGroup` ([`mint_dispatch_group`]),
//!   * resolving the workspace directory.
//!
//! Keeps `mod.rs` focused on the run-time choreography (scan feedback,
//! dispatch verifier, finalise the feature) rather than the
//! deterministic setup boilerplate.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::io::RdfFormat;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use super::bundle::{assemble_bundle, persist_store, resolve_product_root, sha256_hex};
use super::quads::{build_dispatch_quads, build_session_quads};
use super::vocab::{DISPATCH_PREFIX, SESSION_PREFIX};
use super::worker::preflight_implementer;
use super::{DispatchContext, ImplementArgs, IMPLEMENTER_ROLE, IMPLEMENT_GOAL, SLICE1_MODEL_ID};
use crate::core::dispatch::DispatchGroup;
use crate::core::scope::ActiveScope;
use crate::core::verify::chain_integrity::{
    run_chain_integrity_gate, ChainIntegrityError, GateInputs, GateOutcome,
};
use crate::core::vocab::{orchestration_graph, IRI_DEC_DISPATCH, IRI_PROV_USED};
use crate::core::StreamWriter;
use oxigraph::model::{GraphName, NamedNodeRef, Quad};

pub(super) fn prepare_dispatch(workdir: &Path, args: &ImplementArgs) -> Result<DispatchContext> {
    validate_scope(workdir)?;
    let bundle = prepare_bundle(workdir, args)?;
    let (store, writer, dump_path) = load_store_and_writer(workdir)?;

    // FT-047 / ADR-031 — chain-integrity gate fires BEFORE any session
    // PROV-O activity is opened and BEFORE the worker is preflighted.
    // If the gate refuses, no session, no dispatch, no group, no worker
    // invocation.
    let waiver_iri = run_gate(workdir, args, &bundle, &store, &writer, &dump_path)?;

    let worker_argv = preflight_worker(workdir, args)?;
    let iris = mint_dispatch_iris(args, &bundle)?;
    commit_initial_session(&writer, &store, &dump_path, args, &bundle, &iris)?;
    // FT-047 / ADR-031: record waiver in the session's PROV-O chain.
    if let Some(waiver) = &waiver_iri {
        record_waiver_on_session(&writer, &store, &dump_path, &iris, waiver)?;
    }
    // FT-021 / ADR-017: mint the DispatchGroup at command entry, status
    // `awaiting-action`. The state machine transitions on the action
    // worker's terminal outcome below.
    let group = mint_dispatch_group(&writer, &store, &dump_path, args, &iris)?;
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
        worker_argv,
        group,
        waiver_iri,
    })
}

/// FT-016 / TC-049: worker preflight runs BEFORE any session is opened.
/// A missing worker aborts here with the install-hint block and never
/// touches the orchestration graph.
fn preflight_worker(workdir: &Path, args: &ImplementArgs) -> Result<Vec<String>> {
    preflight_implementer(workdir, args.worker_command.as_deref()).map_err(|fail| {
        anyhow!(
            "no worker found for role `{}`. Pre-flight aborted before session open.\n\n{fail}",
            IMPLEMENTER_ROLE
        )
    })
}

/// FT-047 / ADR-031 — fire the chain-integrity gate.
///
/// Returns `Some(waiver_iri)` when a waiver was minted; `None` when
/// coverage was already complete. Errors propagate the structured gate
/// failure (mapped to exit codes by the CLI wrapper).
fn run_gate(
    workdir: &Path,
    args: &ImplementArgs,
    bundle: &PreparedBundle,
    store: &Store,
    writer: &StreamWriter,
    dump_path: &Path,
) -> Result<Option<oxigraph::model::NamedNode>> {
    let outcome = run_chain_integrity_gate(GateInputs {
        workdir,
        product_root: &bundle.product_root,
        store,
        writer,
        feature_short_id: &args.feature_id,
        waiver: args.waiver.as_ref(),
        attributed_to: oxigraph::model::NamedNode::new_unchecked("urn:dec:agent:cli"),
        known_envs: &[],
    })
    .map_err(|err| match err {
        ChainIntegrityError::ChainIntegrity { .. } => anyhow!("{err}"),
        ChainIntegrityError::InvalidWaiverReason(_) => anyhow!("{err}"),
        ChainIntegrityError::Coverage(c) => anyhow!("coverage primitive failed: {c}"),
        ChainIntegrityError::WaiverPersistFailed(p) => {
            anyhow!("waiver persistence failed: {p}")
        }
    })?;
    match outcome {
        GateOutcome::Clean => Ok(None),
        GateOutcome::Waived { waiver } => {
            // The waiver was persisted via StreamWriter; flush the store
            // dump so a subsequent process picks up the waiver record
            // even if we crash before completing the dispatch.
            persist_store(store, dump_path)?;
            Ok(Some(waiver.iri))
        }
    }
}

/// Record `<session> prov:used <waiver>` and `<dispatch> prov:used <waiver>`
/// so the PROV-O chain captures the waiver per FT-047 §State.
fn record_waiver_on_session(
    writer: &StreamWriter,
    store: &Store,
    dump_path: &Path,
    iris: &MintedIris,
    waiver_iri: &oxigraph::model::NamedNode,
) -> Result<()> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let prov_used = NamedNodeRef::new_unchecked(IRI_PROV_USED).into_owned();
    let quads = vec![
        Quad::new(
            iris.session.clone(),
            prov_used.clone(),
            waiver_iri.clone(),
            g.clone(),
        ),
        Quad::new(iris.dispatch.clone(), prov_used, waiver_iri.clone(), g),
    ];
    // Silence the unused-import warning when build features don't pull it.
    let _ = IRI_DEC_DISPATCH;
    let mutation = oxi_events::Mutation::insert(quads.iter().cloned())
        .with_cause("dec implement: prov:used waiver");
    writer
        .commit(mutation)
        .context("recording prov:used waiver on session")?;
    persist_store(store, dump_path)
}

/// Mint a fresh `dec:DispatchGroup` paired with the action session
/// (FT-021 §Behaviour). Persists the store snapshot so the new artifact
/// shows up in subsequent `dec` invocations even if the worker crashes.
fn mint_dispatch_group(
    writer: &StreamWriter,
    store: &Store,
    dump_path: &Path,
    args: &ImplementArgs,
    iris: &MintedIris,
) -> Result<DispatchGroup> {
    let group_uuid = uuid::Uuid::new_v4();
    let group_iri = NamedNode::new(format!(
        "https://decision-cli.dev/ns/dispatch-group/{group_uuid}"
    ))
    .context("minting DispatchGroup IRI")?;
    let group = DispatchGroup::mint(
        writer,
        group_iri,
        iris.session.clone(),
        args.feature_id.clone(),
    )
    .with_context(|| format!("minting DispatchGroup for {}", args.feature_id))?;
    persist_store(store, dump_path)?;
    Ok(group)
}

/// Load the active scope and reject the dispatch if the stream does not
/// authorise the implementer goal ([`IMPLEMENT_GOAL`]).
fn validate_scope(workdir: &Path) -> Result<()> {
    let scope = ActiveScope::load(workdir).map_err(|e| anyhow!("loading active scope: {e}"))?;
    scope
        .validate_goal(IMPLEMENT_GOAL)
        .map_err(|e| anyhow!("goal refused: {e}"))
}

pub(super) struct PreparedBundle {
    pub markdown: String,
    pub hash: String,
    pub iri: String,
    pub product_root: PathBuf,
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

pub(super) struct MintedIris {
    pub session: NamedNode,
    pub dispatch: NamedNode,
    pub bundle_ref: NamedNode,
    pub model_ref: NamedNode,
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
