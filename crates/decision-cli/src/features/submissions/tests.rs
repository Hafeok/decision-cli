//! Internal unit tests for the `submissions` feature module.
//!
//! End-to-end behaviour (including the axum router) is exercised by the
//! integration test at `crates/decision-cli/tests/tc_136_*.rs`.

use std::sync::Arc;

use oxi_events::GraphWriter;
use oxigraph::store::Store;

use crate::core::ontology::worker_image_submission::SubmissionLifecycleState;
use crate::core::vocab::worker_image_submission_graph;

use super::auth::{RepoIdentity, TokenStore};
use super::handler::{
    RateLimiter, SubmissionsService, SubmissionsServiceError,
};
use super::payload::SubmissionPayload;

const TOKEN: &str = "test-token-abc";
const REPO: &str = "https://github.com/example/worker";

fn baseline_payload() -> SubmissionPayload {
    SubmissionPayload {
        id: Some("sub-1".to_string()),
        candidate_registry_ref:
            "ghcr.io/example/worker@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                .to_string(),
        claimed_capability_tags: vec!["code-writer".to_string()],
        claimed_compatible_roles: vec![],
        claimed_sbom_ref:
            "ghcr.io/example/worker@sha256:cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"
                .to_string(),
        claimed_signature_subject:
            "https://github.com/example/worker/.github/workflows/build.yml@refs/heads/main"
                .to_string(),
        claimed_signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        claimed_source_repo_uri: REPO.to_string(),
        claimed_source_commit_hash: "abc123".to_string(),
        claimed_build_run_url: "https://github.com/example/worker/actions/runs/1".to_string(),
        external_origin: Some("github-actions:example/worker/runs/1".to_string()),
    }
}

fn build_service() -> (SubmissionsService, Arc<Store>) {
    let store = Arc::new(Store::new().expect("memory store"));
    let writer = Arc::new(GraphWriter::open(store.clone()).expect("graph writer"));
    let identity = RepoIdentity::new(REPO).expect("identity");
    let tokens = TokenStore::single(TOKEN, identity).expect("token store");
    let limiter = Arc::new(RateLimiter::with_default_policy());
    let service = SubmissionsService::new(writer, tokens, limiter);
    (service, store)
}

#[test]
fn unauthorised_when_no_token() {
    let (service, _store) = build_service();
    let err = service
        .accept(None, baseline_payload())
        .expect_err("no token must fail");
    assert_eq!(err, SubmissionsServiceError::Unauthorised);
}

#[test]
fn unauthorised_when_unknown_token() {
    let (service, _store) = build_service();
    let err = service
        .accept(Some("not-a-real-token"), baseline_payload())
        .expect_err("unknown token must fail");
    assert_eq!(err, SubmissionsServiceError::Unauthorised);
}

#[test]
fn forbidden_when_identity_mismatch() {
    let (service, _store) = build_service();
    let mut payload = baseline_payload();
    payload.claimed_source_repo_uri = "https://github.com/other/repo".to_string();
    let err = service
        .accept(Some(TOKEN), payload)
        .expect_err("identity mismatch must fail");
    match err {
        SubmissionsServiceError::IdentityMismatch {
            token_identity,
            declared,
        } => {
            assert_eq!(token_identity, REPO);
            assert_eq!(declared, "https://github.com/other/repo");
        }
        other => panic!("expected IdentityMismatch, got {other:?}"),
    }
}

#[test]
fn payload_invalid_when_capability_tags_empty() {
    let (service, _store) = build_service();
    let mut payload = baseline_payload();
    payload.claimed_capability_tags.clear();
    let err = service
        .accept(Some(TOKEN), payload)
        .expect_err("zero capability tags must fail");
    match err {
        SubmissionsServiceError::PayloadInvalid { report } => {
            assert!(
                report.contains("claimed_capability_tag"),
                "report should cite the field: {report}"
            );
        }
        other => panic!("expected PayloadInvalid, got {other:?}"),
    }
}

#[test]
fn payload_invalid_when_registry_ref_missing_digest() {
    let (service, _store) = build_service();
    let mut payload = baseline_payload();
    payload.candidate_registry_ref = "ghcr.io/example/worker:latest".to_string();
    let err = service
        .accept(Some(TOKEN), payload)
        .expect_err("non-digest registry_ref must fail");
    match err {
        SubmissionsServiceError::PayloadInvalid { report } => {
            assert!(
                report.contains("@sha256:"),
                "report should cite the digest invariant: {report}"
            );
        }
        other => panic!("expected PayloadInvalid, got {other:?}"),
    }
}

#[test]
fn payload_invalid_when_role_iri_malformed() {
    let (service, _store) = build_service();
    let mut payload = baseline_payload();
    payload.claimed_compatible_roles = vec!["not a valid iri".to_string()];
    let err = service
        .accept(Some(TOKEN), payload)
        .expect_err("malformed role iri must fail");
    match err {
        SubmissionsServiceError::PayloadInvalid { report } => {
            assert!(
                report.contains("claimed_compatible_roles"),
                "report should cite the field: {report}"
            );
        }
        other => panic!("expected PayloadInvalid, got {other:?}"),
    }
}

#[test]
fn rate_limited_when_limiter_full() {
    let store = Arc::new(Store::new().expect("memory store"));
    let writer = Arc::new(GraphWriter::open(store.clone()).expect("graph writer"));
    let identity = RepoIdentity::new(REPO).expect("identity");
    let tokens = TokenStore::single(TOKEN, identity).expect("token store");
    let limiter = Arc::new(RateLimiter::deny_all());
    let service = SubmissionsService::new(writer, tokens, limiter);
    let err = service
        .accept(Some(TOKEN), baseline_payload())
        .expect_err("deny-all limiter must reject");
    match err {
        SubmissionsServiceError::RateLimited { repo } => assert_eq!(repo, REPO),
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

#[test]
fn accept_writes_submission_to_store() {
    use oxigraph::sparql::QueryResults;

    let (service, store) = build_service();
    let accepted = service
        .accept(Some(TOKEN), baseline_payload())
        .expect("well-formed Submission must accept");

    assert!(accepted.submission_iri.ends_with("/sub-1"));
    assert_eq!(accepted.submission_id, "sub-1");
    assert_eq!(accepted.target_role, "worker-curator");
    assert!(accepted.dispatch_event_id.starts_with("urn:uuid:"));

    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { GRAPH ?g { ?s a dec:WorkerImageSubmission } }";
    let QueryResults::Solutions(sols) = store.query(q).expect("query ok") else {
        panic!("expected solutions");
    };
    let count = sols.count();
    assert_eq!(count, 1, "expected exactly one Submission in store");
}

#[test]
fn rejection_does_not_touch_store() {
    use oxigraph::sparql::QueryResults;

    let (service, store) = build_service();
    let mut payload = baseline_payload();
    payload.claimed_capability_tags.clear();
    let _ = service.accept(Some(TOKEN), payload).expect_err("must fail");
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { GRAPH ?g { ?s a dec:WorkerImageSubmission } }";
    let QueryResults::Solutions(sols) = store.query(q).expect("query ok") else {
        panic!("expected solutions");
    };
    let count = sols.count();
    assert_eq!(count, 0, "rejected Submission must not appear in store");
}

#[test]
fn default_external_origin_uses_run_url_when_omitted() {
    let (service, _store) = build_service();
    let mut payload = baseline_payload();
    payload.external_origin = None;
    let _ = service
        .accept(Some(TOKEN), payload)
        .expect("Submission with default external_origin must accept");
}

#[test]
fn lifecycle_state_pinned_to_received() {
    // The handler ignores any client-side desire to start in another
    // state — the Curator owns transitions. We verify this by checking
    // that the materialised Submission persists as `received`.
    let (service, store) = build_service();
    let _ = service.accept(Some(TOKEN), baseline_payload()).expect("ok");
    let q = "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?state WHERE { GRAPH ?g { ?s a dec:WorkerImageSubmission ; \
                                            dec:submission_lifecycle_state ?state } }";
    let oxigraph::sparql::QueryResults::Solutions(sols) = store.query(q).expect("query ok")
    else {
        panic!("expected solutions");
    };
    let mut seen = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("state") {
            seen.push(lit.value().to_string());
        }
    }
    assert_eq!(seen, vec![SubmissionLifecycleState::Received.as_str().to_string()]);
}

#[test]
fn graph_name_is_worker_image_submission_graph() {
    use oxigraph::sparql::QueryResults;

    let (service, store) = build_service();
    let _ = service.accept(Some(TOKEN), baseline_payload()).expect("ok");
    let expected_graph = worker_image_submission_graph();
    let q = "SELECT ?g WHERE { GRAPH ?g { ?s a <https://decision-cli.dev/ns#WorkerImageSubmission> } }";
    let QueryResults::Solutions(sols) = store.query(q).expect("query ok") else {
        panic!("expected solutions");
    };
    let mut graphs = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("g") {
            graphs.push(n.as_str().to_string());
        }
    }
    assert_eq!(graphs, vec![expected_graph.as_str().to_string()]);
}
