//! TC-129 — WorkerImageSubmission validates as a BoundaryArtifact.
//!
//! Validates: FT-087 · ADR-040 · ADR-055.
//! Spec: `.product/tests/TC-129-workerimagesubmission-validates-as-a-boundaryartif.md`
//!
//! Three claims this integration test pins down end-to-end against a real
//! Oxigraph store:
//!
//! 1. A well-formed `dec:WorkerImageSubmission` round-trips through RDF
//!    serialisation and admits both the WorkerImageSubmission field-level
//!    SHACL validator (FT-087) and the BoundaryArtifact escape-hatch
//!    validator (FT-071 / ADR-040).
//! 2. The Submission's serialised form carries the BoundaryArtifact class
//!    membership the per-type shape's `sh:or` requires — via the
//!    co-declared `dec:InitialRequest` `rdf:type` and the subClassOf chain
//!    declared by the embedded ontology.
//! 3. SHACL refuses Submissions missing required fields (digest-pinned
//!    candidate_registry_ref, at least one capability tag, valid lifecycle
//!    state, non-empty external_origin).

use oxigraph::model::{Literal, NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::ontology::boundary_artifact::validate_boundary_artifact;
use decision_cli::core::ontology::worker_image_submission::{
    validate_quads, SubmissionLifecycleState, WorkerImageSubmission,
};
use decision_cli::vocab::{
    worker_image_submission_graph, IRI_DEC_CANDIDATE_REGISTRY_REF,
    IRI_DEC_SUBMISSION_LIFECYCLE_STATE, IRI_DEC_WORKER_IMAGE_SUBMISSION,
};

fn well_formed_submission(id: &str) -> WorkerImageSubmission {
    WorkerImageSubmission {
        id: id.to_string(),
        candidate_registry_ref: format!(
            "ghcr.io/example/{id}@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        ),
        claimed_capability_tags: vec!["code-writer".to_string(), "implementer".to_string()],
        claimed_compatible_roles: Vec::new(),
        claimed_sbom_ref: format!(
            "ghcr.io/example/{id}@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef:sbom"
        ),
        claimed_signature_subject: format!(
            "https://github.com/example/{id}/.github/workflows/build.yml@refs/heads/main"
        ),
        claimed_signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
        claimed_source_repo_uri: format!("https://github.com/example/{id}"),
        claimed_source_commit_hash: "abc123def456".to_string(),
        claimed_build_run_url: format!("https://github.com/example/{id}/actions/runs/123"),
        lifecycle_state: SubmissionLifecycleState::Received,
        external_origin: format!("github-actions:example/{id}/runs/123"),
        produced_workerimage: None,
        produced_feedback: None,
    }
}

fn load_into_store(store: &Store, sub: &WorkerImageSubmission) {
    let quads = sub.to_quads(worker_image_submission_graph());
    for q in &quads {
        store.insert(q).expect("insert quad");
    }
}

#[test]
fn shacl_admits_well_formed_submission() {
    let sub = well_formed_submission("sub-001");
    let quads = sub.to_quads(worker_image_submission_graph());
    validate_quads(&quads).expect("well-formed Submission must pass field-level SHACL");
}

#[test]
fn boundary_artifact_validator_admits_well_formed_submission() {
    let sub = well_formed_submission("sub-001");
    let quads = sub.to_quads(worker_image_submission_graph());
    validate_boundary_artifact(&quads, &sub.iri())
        .expect("Submission carries external_origin so the BoundaryArtifact shape passes");
}

#[test]
fn round_trips_through_store() {
    let store = Store::new().expect("memory store");
    let sub = well_formed_submission("sub-001");
    load_into_store(&store, &sub);

    // SHACL passes against the serialised quads.
    let serialised = sub.to_quads(worker_image_submission_graph());
    validate_quads(&serialised).expect("round-tripped Submission passes field SHACL");

    // The Submission is queryable by class.
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?s WHERE {{ GRAPH ?g {{ ?s a <{IRI_DEC_WORKER_IMAGE_SUBMISSION}> }} }}",
    );
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("query ok") else {
        panic!("expected solutions");
    };
    let mut count = 0;
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(Term::NamedNode(n)) = sol.get("s") {
            assert_eq!(n, &sub.iri());
            count += 1;
        }
    }
    assert_eq!(count, 1, "expected exactly one Submission in store");
}

#[test]
fn submission_carries_boundary_class_membership() {
    // The Submission MUST be classified as a BoundaryArtifact (or a
    // subclass thereof) so the per-type shape's `sh:or [ sh:class
    // dec:BoundaryArtifact ]` branch is satisfied. The serialised form
    // declares `dec:InitialRequest` explicitly; the embedded ontology
    // declares dec:InitialRequest rdfs:subClassOf dec:BoundaryArtifact
    // and dec:WorkerImageSubmission rdfs:subClassOf dec:InitialRequest.
    let sub = well_formed_submission("sub-001");
    let quads = sub.to_quads(worker_image_submission_graph());

    let rdf_type = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    let mut declared_types: Vec<&str> = quads
        .iter()
        .filter(|q| q.predicate.as_str() == rdf_type)
        .filter_map(|q| match &q.object {
            Term::NamedNode(n) => Some(n.as_str()),
            _ => None,
        })
        .collect();
    declared_types.sort();

    assert!(
        declared_types.contains(&"https://decision-cli.dev/ns#WorkerImageSubmission"),
        "missing rdf:type dec:WorkerImageSubmission: {declared_types:?}"
    );
    assert!(
        declared_types.contains(&"https://decision-cli.dev/ns#InitialRequest"),
        "missing rdf:type dec:InitialRequest (BoundaryArtifact subclass): {declared_types:?}"
    );
}

#[test]
fn shacl_rejects_submission_missing_registry_digest() {
    let sub = well_formed_submission("sub-001");
    let mut quads = sub.to_quads(worker_image_submission_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_CANDIDATE_REGISTRY_REF {
            q.object = Literal::new_simple_literal("ghcr.io/example/worker:latest").into();
        }
    }
    let err = validate_quads(&quads)
        .expect_err("non-digest candidate_registry_ref must fail SHACL");
    assert!(err.report.contains("@sha256:"), "{}", err.report);
}

#[test]
fn shacl_rejects_submission_with_zero_capability_tags() {
    let mut sub = well_formed_submission("sub-001");
    sub.claimed_capability_tags.clear();
    let quads = sub.to_quads(worker_image_submission_graph());
    let err = validate_quads(&quads)
        .expect_err("zero claimed_capability_tags must fail SHACL");
    assert!(err.report.contains("claimed_capability_tag"), "{}", err.report);
}

#[test]
fn shacl_rejects_submission_with_unknown_lifecycle_state() {
    let sub = well_formed_submission("sub-001");
    let mut quads: Vec<Quad> = sub.to_quads(worker_image_submission_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_SUBMISSION_LIFECYCLE_STATE {
            q.object = Literal::new_simple_literal("retired").into();
        }
    }
    let err = validate_quads(&quads).expect_err("unknown lifecycle state must fail SHACL");
    assert!(
        err.report.contains("submission_lifecycle_state"),
        "{}",
        err.report
    );
}

#[test]
fn shacl_rejects_submission_missing_external_origin() {
    let mut sub = well_formed_submission("sub-001");
    sub.external_origin = String::new();
    let quads = sub.to_quads(worker_image_submission_graph());
    let err = validate_quads(&quads).expect_err("empty external_origin must fail SHACL");
    assert!(err.report.contains("external_origin"), "{}", err.report);
}

#[test]
fn lifecycle_admitted_round_trips() {
    // The lifecycle vocabulary is fixed at four values; admitted is the
    // terminal success state. Round-trip through the type to confirm.
    let mut sub = well_formed_submission("sub-001");
    sub.lifecycle_state = SubmissionLifecycleState::Admitted;
    sub.produced_workerimage = Some(NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/worker-image/example/v1.0.0",
    ));
    let quads = sub.to_quads(worker_image_submission_graph());
    validate_quads(&quads)
        .expect("Submission in admitted state with produced_workerimage must pass SHACL");

    // The produced_workerimage edge is serialised.
    let pred = "https://decision-cli.dev/ns#produced_workerimage";
    let count = quads.iter().filter(|q| q.predicate.as_str() == pred).count();
    assert_eq!(count, 1, "expected exactly one produced_workerimage edge");
}

#[test]
fn raw_class_iri_matches_vocab() {
    // Belt-and-braces: ensure the test's own predicate references stay in
    // sync with the public vocab IRI.
    assert_eq!(
        IRI_DEC_WORKER_IMAGE_SUBMISSION,
        "https://decision-cli.dev/ns#WorkerImageSubmission"
    );
}

/// Single-entry checkpoint test — the product-cli runner (cargo-test
/// runner) looks up TC-129 by this function name in `tests/*.rs` and
/// flips the TC to `passing` only when this test runs and exits 0. The
/// body re-runs the structural claims of TC-129 so this one function
/// reproduces the exit-criterion in one shot.
#[test]
fn tc_129_workerimagesubmission_validates_as_a_boundaryartif() {
    let sub = well_formed_submission("sub-tc-129");
    let quads = sub.to_quads(worker_image_submission_graph());

    // 1. Field-level SHACL admits the Submission.
    validate_quads(&quads).expect("well-formed Submission must pass field-level SHACL");

    // 2. BoundaryArtifact validator admits the Submission's external_origin.
    validate_boundary_artifact(&quads, &sub.iri())
        .expect("Submission carries external_origin so :BoundaryArtifactShape passes");

    // 3. Submission carries both rdf:type dec:WorkerImageSubmission AND
    //    rdf:type dec:InitialRequest (subclass of dec:BoundaryArtifact).
    let rdf_type = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    let types: Vec<&str> = quads
        .iter()
        .filter(|q| q.predicate.as_str() == rdf_type)
        .filter_map(|q| match &q.object {
            Term::NamedNode(n) => Some(n.as_str()),
            _ => None,
        })
        .collect();
    assert!(types.contains(&IRI_DEC_WORKER_IMAGE_SUBMISSION));
    assert!(types.contains(&"https://decision-cli.dev/ns#InitialRequest"));

    // 4. SHACL rejects a Submission missing the digest pin.
    let mut bad = sub.clone();
    bad.candidate_registry_ref = "ghcr.io/example/worker:latest".to_string();
    let err = validate_quads(&bad.to_quads(worker_image_submission_graph()))
        .expect_err("missing digest must fail SHACL");
    assert!(err.report.contains("@sha256:"), "{}", err.report);

    // 5. SHACL rejects a Submission with zero capability tags.
    let mut bad = sub.clone();
    bad.claimed_capability_tags.clear();
    let err = validate_quads(&bad.to_quads(worker_image_submission_graph()))
        .expect_err("zero capability tags must fail SHACL");
    assert!(err.report.contains("claimed_capability_tag"), "{}", err.report);

    // 6. SHACL rejects an unknown lifecycle state.
    let mut quads = sub.to_quads(worker_image_submission_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_SUBMISSION_LIFECYCLE_STATE {
            q.object = Literal::new_simple_literal("retired").into();
        }
    }
    let err = validate_quads(&quads).expect_err("unknown lifecycle must fail SHACL");
    assert!(err.report.contains("submission_lifecycle_state"), "{}", err.report);

    // 7. SHACL rejects an empty external_origin.
    let mut bad = sub.clone();
    bad.external_origin = String::new();
    let err = validate_quads(&bad.to_quads(worker_image_submission_graph()))
        .expect_err("empty external_origin must fail SHACL");
    assert!(err.report.contains("external_origin"), "{}", err.report);

    // 8. An admitted Submission with a produced_workerimage edge round-trips
    //    cleanly through SHACL and through the in-memory store.
    let mut admitted = sub.clone();
    admitted.lifecycle_state = SubmissionLifecycleState::Admitted;
    admitted.produced_workerimage = Some(NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/worker-image/example/v1.0.0",
    ));
    let admitted_quads = admitted.to_quads(worker_image_submission_graph());
    validate_quads(&admitted_quads).expect("admitted Submission must pass SHACL");
    let store = Store::new().expect("memory store");
    for q in &admitted_quads {
        store.insert(q).expect("insert quad");
    }
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?s WHERE {{ GRAPH ?g {{ ?s a <{IRI_DEC_WORKER_IMAGE_SUBMISSION}> }} }}",
    );
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("query ok") else {
        panic!("expected solutions");
    };
    let mut count = 0;
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(Term::NamedNode(n)) = sol.get("s") {
            assert_eq!(n, &admitted.iri());
            count += 1;
        }
    }
    assert_eq!(count, 1, "expected exactly one Submission in store");
}
