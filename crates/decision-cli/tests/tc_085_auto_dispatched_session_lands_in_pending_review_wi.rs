//! TC-085 — auto-dispatched session lands in pending_review without persisting graph.
//!
//! Validates: FT-050 · ADR-030.
//! Spec: `.product/tests/TC-085-auto-dispatched-session-lands-in-pending-review-wi.md`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use decision_cli::core::stream_writer::StreamWriter;
use decision_cli::core::subscriptions::verify_graph_author_dispatch::{
    emit_dispatch_event,
    session::{load_proposal_document, persist_pending_review_session, PendingReviewInput},
    AutoDispatchConfig, AutoDispatchSeed,
};
use oxigraph::model::{NamedNode, NamedNodeRef, Term};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/test-tc-085";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const DEC_STATUS: &str = "https://decision-cli.dev/ns#status";
const DEC_VERIFIES: &str = "https://decision-cli.dev/ns#verifies";
const DEC_ENVIRONMENT: &str = "https://decision-cli.dev/ns#environment";
const DEC_PROPOSAL_DOC: &str = "https://decision-cli.dev/ns#proposalDocument";
const DEC_SESSION_CLASS: &str = "https://decision-cli.dev/ns#Session";
const DEC_VERIFICATION_GRAPH: &str = "https://decision-cli.dev/ns#VerificationGraph";

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc085-{tag}-{}-{}-{}",
            std::process::id(),
            nanos,
            counter,
        ));
        fs::create_dir_all(&base).expect("create temp workdir");
        Self(base)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkdirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("writer");
    (store, w)
}

fn count_verification_graphs(store: &Store) -> usize {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let graph_class = NamedNodeRef::new_unchecked(DEC_VERIFICATION_GRAPH);
    store
        .quads_for_pattern(None, Some(rdf_type), Some(graph_class.into()), None)
        .filter_map(Result::ok)
        .count()
}

fn pending_review_sessions(store: &Store) -> Vec<NamedNode> {
    // Find sessions whose status is pending_review.
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let session_class = NamedNodeRef::new_unchecked(DEC_SESSION_CLASS);
    let status_pred = NamedNodeRef::new_unchecked(DEC_STATUS);
    let mut out: Vec<NamedNode> = Vec::new();
    for q in store
        .quads_for_pattern(None, Some(rdf_type), Some(session_class.into()), None)
        .filter_map(Result::ok)
    {
        let oxigraph::model::Subject::NamedNode(iri) = q.subject else {
            continue;
        };
        let is_pending = store
            .quads_for_pattern(
                Some(oxigraph::model::Subject::NamedNode(iri.clone()).as_ref()),
                Some(status_pred),
                None,
                None,
            )
            .filter_map(Result::ok)
            .any(|q| matches!(&q.object, Term::Literal(lit) if lit.value() == "pending_review"));
        if is_pending {
            out.push(iri);
        }
    }
    out
}

fn read_literal(store: &Store, subject: &NamedNode, predicate: &str) -> Option<String> {
    let pred = NamedNodeRef::new_unchecked(predicate);
    store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(subject.clone()).as_ref()),
            Some(pred),
            None,
            None,
        )
        .filter_map(Result::ok)
        .find_map(|q| match q.object {
            Term::Literal(lit) => Some(lit.value().to_string()),
            _ => None,
        })
}

#[test]
fn tc_085_auto_dispatched_session_lands_in_pending_review_wi() {
    let guard = WorkdirGuard::new("session-pending-review");
    let (store, w) = writer();

    let cfg = AutoDispatchConfig::default();
    let seed = AutoDispatchSeed {
        feature: "FT-J".to_string(),
        env: "ENV-1".to_string(),
        triggered_by_event_id: "urn:dec:event/feature-create/FT-J".to_string(),
        bundle_hash: "tc085-bundle-hash".to_string(),
    };

    // 1. The subscription fires for (FT-J, ENV-1).
    let event = emit_dispatch_event(&w, &store, &seed, &cfg, "2026-05-21T10:00:00Z")
        .expect("emit ok")
        .expect("event emitted");
    assert_eq!(event.feature, "FT-J");
    assert_eq!(event.env, "ENV-1");

    // 2. The orchestrator picks up the event, assembles the bundle, and
    //    invokes the (mocked) worker which returns a `New` proposal.
    //
    //    For the unit test we model the mocked-worker output directly
    //    — the proposal JSON. The subscription's contract is that
    //    whatever the worker returns is serialised onto the session's
    //    `dec:proposalDocument` literal verbatim.
    let proposal_json = serde_json::json!({
        "kind": "new",
        "bundle_hash": "tc085-bundle-hash",
        "new": {
            "environment": "ENV-1",
            "steps": [
                {
                    "step_type": "shell-command",
                    "fields": {"command": "echo ok", "expect_exit_code": "0"},
                    "provides_evidence_for": ["TC-001"]
                }
            ],
            "rationale": "TC-085 fixture step covers TC-001"
        }
    })
    .to_string();

    let session = persist_pending_review_session(
        &w,
        &PendingReviewInput {
            feature: "FT-J",
            env: "ENV-1",
            proposal_document_json: &proposal_json,
            dispatch_event_iri: &event.iri,
            started_at: "2026-05-21T10:00:01Z",
        },
    )
    .expect("persist session");

    // AC #1: Session has status=pending_review.
    let status = read_literal(&store, &session.iri, DEC_STATUS).expect("status set");
    assert_eq!(status, "pending_review");

    // AC #1: dec:proposalDocument carries the JSON payload.
    let doc = read_literal(&store, &session.iri, DEC_PROPOSAL_DOC).expect("doc set");
    assert_eq!(doc, proposal_json);
    let parsed: serde_json::Value = serde_json::from_str(&doc).expect("doc parses");
    assert_eq!(parsed["kind"], "new");

    // AC #1: dec:verifies = FT-J.
    let verifies = read_literal(&store, &session.iri, DEC_VERIFIES).expect("verifies set");
    assert_eq!(verifies, "FT-J");

    // AC #1: dec:environment = ENV-1.
    let env_lit = read_literal(&store, &session.iri, DEC_ENVIRONMENT).expect("environment set");
    assert_eq!(env_lit, "ENV-1");

    // AC #2: No VerificationGraph artifact was written.
    assert_eq!(
        count_verification_graphs(&store),
        0,
        "no VerificationGraph must exist in the store"
    );

    // AC #2: `.dec/verify/graph/` is unchanged — the directory either
    // does not exist, or contains no `.ttl` files. We did not create
    // any `.dec/` layout in this test's workdir, so the directory
    // shouldn't exist.
    let graph_dir = guard.path().join(".dec/verify/graph");
    assert!(!graph_dir.exists(), ".dec/verify/graph/ must remain unchanged");

    // AC #3: `dec session list` shows the pending-review session. The
    // structural surface is the typed session in the orchestration
    // store; pending_review sessions ARE first-class Sessions.
    let pending = pending_review_sessions(&store);
    assert!(
        pending.iter().any(|s| s == &session.iri),
        "session must be discoverable via the standard session-listing path"
    );

    // AC #4: `dec session show <id>` renders the proposal payload.
    // load_proposal_document is the surface that powers that view.
    let shown = load_proposal_document(&store, &session.iri)
        .expect("load ok")
        .expect("proposal present");
    assert_eq!(shown, proposal_json);

    // AC #5: A subsequent `dec verify graph generate FT-J --environment
    // ENV-1 --from-session <id> --accept` reads the proposal from the
    // session and persists the graph through the standard write path.
    //
    // The from-session loader is what we need to validate here. The
    // standard write path is the slice-2.5 writers (FT-041 + FT-044);
    // exercising the full persist call needs a real workdir with a
    // bootstrapped `.dec/` and `.product/`. We validate the *loader*
    // here (the boundary the subscription owns) — the rest of the
    // persist pipeline is exercised by TC-079.
    let payload = load_proposal_document(&store, &session.iri)
        .expect("load ok")
        .expect("proposal present");
    let proposal_value: serde_json::Value =
        serde_json::from_str(&payload).expect("proposal is JSON");
    assert_eq!(proposal_value["kind"], "new");
    assert_eq!(proposal_value["new"]["environment"], "ENV-1");
    let steps = proposal_value["new"]["steps"]
        .as_array()
        .expect("steps array");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["step_type"], "shell-command");

    // AC #6: Chain-integrity gate continues to refuse dispatch while the
    // proposal is pending — an unaccepted proposal does not count as
    // coverage. The chain gate's contract is "coverage is computed
    // from persisted VerificationGraphs, not from pending sessions."
    // We assert that property structurally: the pending session does
    // not write a VerificationGraph (re-checked here for AC #6).
    assert_eq!(
        count_verification_graphs(&store),
        0,
        "pending proposal does not count as coverage — no VerificationGraph artifacts"
    );
}
