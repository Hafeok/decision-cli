//! TC-128 — WorkerImage artifact validates and is discoverable by capability tag.
//!
//! Validates: FT-086 · ADR-055.
//! Spec: `.product/tests/TC-128-workerimage-artifact-validates-and-is-discoverable.md`
//!
//! Three claims this integration test pins down end-to-end against a real
//! Oxigraph store:
//!
//! 1. A well-formed `dec:WorkerImage` admits via SHACL and round-trips
//!    through RDF serialisation back to an equal in-memory struct.
//! 2. The same image, after being persisted into a store, is discoverable
//!    via [`query_by_capability_tag`] for every tag it claimed.
//! 3. Eligibility filtering works: a `qualified` image surfaces under
//!    `EligibilityStatus::Qualified` but not under `Pulled`.
//! 4. SHACL refuses an image missing required fields (registry digest,
//!    capability tag, semver version).

use oxigraph::model::{GraphName, NamedNode, Quad};
use oxigraph::store::Store;

use decision_cli::core::ontology::worker_image::{
    query_by_capability_tag, query_by_eligibility_status, query_by_id, validate_quads,
    EligibilityStatus, WorkerImage,
};
use decision_cli::vocab::{worker_image_graph, IRI_DEC_CAPABILITY_TAG};

fn qualified_image(id: &str, tags: &[&str]) -> WorkerImage {
    WorkerImage {
        id: id.to_string(),
        name: format!("Image {id}"),
        version: "1.0.0".to_string(),
        registry_ref: format!(
            "ghcr.io/example/{id}@sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        ),
        capability_tags: tags.iter().map(|s| (*s).to_string()).collect(),
        compatible_roles: Vec::new(),
        signed_by_subject: format!(
            "https://github.com/example/{id}/.github/workflows/build.yml@refs/heads/main"
        ),
        signed_by_issuer: "https://token.actions.githubusercontent.com".to_string(),
        sbom_ref: format!(
            "ghcr.io/example/{id}@sha256:cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"
        ),
        conformance_audits: Vec::new(),
        eligibility_status: EligibilityStatus::Qualified,
        source_repo_uri: format!("https://github.com/example/{id}"),
        source_commit_hash: "abc123def456".to_string(),
        build_run_url: format!("https://github.com/example/{id}/actions/runs/1"),
    }
}

fn load_into_store(store: &Store, image: &WorkerImage) {
    let quads = image.to_quads(worker_image_graph());
    for q in &quads {
        store.insert(q).expect("insert quad");
    }
}

#[test]
fn shacl_admits_well_formed_image() {
    let img = qualified_image("code-writer-impl", &["code-writer", "implementer"]);
    let quads = img.to_quads(worker_image_graph());
    validate_quads(&quads).expect("well-formed image must pass SHACL");
}

#[test]
fn discoverable_by_capability_tag() {
    let store = Store::new().expect("memory store");
    let code_writer = qualified_image("code-writer-impl", &["code-writer", "implementer"]);
    let verifier = qualified_image("verifier-impl", &["verifier"]);
    let general = qualified_image("general-impl", &["code-writer", "verifier", "implementer"]);

    load_into_store(&store, &code_writer);
    load_into_store(&store, &verifier);
    load_into_store(&store, &general);

    // "code-writer" tag matches code-writer-impl and general-impl.
    let hits = query_by_capability_tag(&store, "code-writer").expect("query ok");
    let ids: Vec<&str> = hits.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["code-writer-impl", "general-impl"]);
    for img in &hits {
        assert!(img.capability_tags.iter().any(|t| t == "code-writer"));
    }

    // "verifier" tag matches verifier-impl and general-impl.
    let hits = query_by_capability_tag(&store, "verifier").expect("query ok");
    let ids: Vec<&str> = hits.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["general-impl", "verifier-impl"]);

    // Unknown tag returns the empty set.
    let hits = query_by_capability_tag(&store, "does-not-exist").expect("query ok");
    assert!(hits.is_empty(), "{hits:?}");
}

#[test]
fn discoverable_by_id_returns_round_tripped_struct() {
    let store = Store::new().expect("memory store");
    let original = qualified_image("code-writer-impl", &["code-writer"]);
    load_into_store(&store, &original);

    let hits = query_by_id(&store, "code-writer-impl").expect("query ok");
    assert_eq!(hits.len(), 1, "expected exactly one image, got {hits:?}");
    assert_eq!(hits[0], original);
}

#[test]
fn eligibility_filter_separates_qualified_from_pulled() {
    let store = Store::new().expect("memory store");
    let qualified = qualified_image("code-writer-impl", &["code-writer"]);
    let mut pulled = qualified_image("pulled-impl", &["code-writer"]);
    pulled.eligibility_status = EligibilityStatus::Pulled;

    load_into_store(&store, &qualified);
    load_into_store(&store, &pulled);

    // "qualified" surfaces only the qualified image.
    let qualified_hits =
        query_by_eligibility_status(&store, EligibilityStatus::Qualified).expect("query ok");
    let ids: Vec<&str> = qualified_hits.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["code-writer-impl"]);

    // "pulled" surfaces only the pulled image.
    let pulled_hits =
        query_by_eligibility_status(&store, EligibilityStatus::Pulled).expect("query ok");
    let ids: Vec<&str> = pulled_hits.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["pulled-impl"]);
}

#[test]
fn the_canonical_capability_tag_query_finds_qualified_images() {
    // The success-criteria scenario from the FT-086 spec:
    // "find qualified images claiming capability tag X".
    let store = Store::new().expect("memory store");
    let qualified = qualified_image("code-writer-impl", &["code-writer"]);
    let mut pulled = qualified_image("pulled-impl", &["code-writer"]);
    pulled.eligibility_status = EligibilityStatus::Pulled;

    load_into_store(&store, &qualified);
    load_into_store(&store, &pulled);

    let by_tag = query_by_capability_tag(&store, "code-writer").expect("query ok");
    let qualified_only: Vec<&WorkerImage> = by_tag
        .iter()
        .filter(|i| i.eligibility_status == EligibilityStatus::Qualified)
        .collect();
    assert_eq!(qualified_only.len(), 1, "{by_tag:?}");
    assert_eq!(qualified_only[0].id, "code-writer-impl");
}

#[test]
fn shacl_rejects_image_missing_registry_digest() {
    let mut img = qualified_image("code-writer-impl", &["code-writer"]);
    img.registry_ref = "ghcr.io/example/worker:latest".to_string();
    let quads = img.to_quads(worker_image_graph());
    let err = validate_quads(&quads).expect_err("non-digest registry_ref must fail SHACL");
    assert!(err.report.contains("@sha256:"), "{}", err.report);
}

#[test]
fn shacl_rejects_image_with_zero_capability_tags() {
    let mut img = qualified_image("code-writer-impl", &[]);
    img.capability_tags.clear();
    let quads = img.to_quads(worker_image_graph());
    let err = validate_quads(&quads).expect_err("zero capability_tags must fail SHACL");
    assert!(err.report.contains("capability_tag"), "{}", err.report);
}

#[test]
fn shacl_rejects_image_with_unknown_eligibility() {
    // Construct quads by hand with a bad eligibility literal — the
    // enum type prevents this in the well-typed path, but the validator
    // is the final gate for graph mutations from arbitrary writers.
    let img = qualified_image("code-writer-impl", &["code-writer"]);
    let mut quads: Vec<Quad> = img.to_quads(worker_image_graph());
    let subject = img.iri();
    let elig = NamedNode::new_unchecked(decision_cli::vocab::IRI_DEC_ELIGIBILITY_STATUS);
    let graph: GraphName = worker_image_graph().into_owned().into();
    quads.retain(|q| q.predicate != elig);
    quads.push(Quad::new(
        subject,
        elig,
        oxigraph::model::Literal::new_simple_literal("retired"),
        graph,
    ));
    let err = validate_quads(&quads).expect_err("unknown eligibility must fail SHACL");
    assert!(err.report.contains("eligibility"), "{}", err.report);
}

#[test]
fn raw_capability_tag_predicate_iri_matches_vocab() {
    // Belt-and-braces: ensure the test's own predicate references stay in
    // sync with the public vocab IRI.
    assert_eq!(
        IRI_DEC_CAPABILITY_TAG,
        "https://decision-cli.dev/ns#capability_tag"
    );
}
