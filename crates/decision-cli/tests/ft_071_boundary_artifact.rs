//! TC-121 — BoundaryArtifact class + per-subclass shapes (FT-071 / ADR-040 / ADR-042).
//!
//! Exit criterion for FT-071: declaring an artifact as `:BoundaryArtifact`
//! (or one of its four subclasses) satisfies the type's motivational
//! `sh:or` requirement without needing any motivational predicate edge,
//! provided `dec:external_origin` is present. Concretely:
//!
//! 1. A fixture `:Feature` artifact carrying only a mechanical block +
//!    `rdf:type dec:Feature, dec:InitialRequest` + `dec:external_origin
//!    "chat-transcript:..."` validates against a per-type shape whose
//!    motivational `sh:or` begins with the BoundaryArtifact branch
//!    (this test models that branch via the slice-1 ShipMechanism since
//!    FT-072 has not yet landed the per-type shape catalog).
//! 2. The same artifact stripped of `dec:external_origin` fails
//!    `:BoundaryArtifactShape` validation with the violation report
//!    naming the property path.
//! 3. A `:MigrationBackfill` instance lacking `dec:isMigrationBackfill
//!    true` fails the `:MigrationBackfillShape` extension.
//! 4. The four BoundaryArtifact subclasses (`SensingActionOutput`,
//!    `InitialRequest`, `BootstrapArtifact`, `MigrationBackfill`) are
//!    loaded as `rdfs:subClassOf dec:BoundaryArtifact`, verified by
//!    SPARQL query over the bootstrap shapes graph.

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::sparql::QueryResults;

use decision_cli::core::ontology::boundary_artifact::{
    validate_boundary_artifact, validate_migration_backfill,
};
use decision_cli::core::ontology::{
    OntologyHandle, BOOTSTRAP_ARTIFACT, BOUNDARY_ARTIFACT_CLASS, BOUNDARY_ARTIFACT_SHAPE,
    BOUNDARY_ARTIFACT_SHAPES_TTL, BOUNDARY_ARTIFACT_SUBCLASSES, EXTERNAL_ORIGIN_PROP,
    INITIAL_REQUEST, IS_MIGRATION_BACKFILL_PROP, MIGRATION_BACKFILL, MIGRATION_BACKFILL_SHAPE,
    SENSING_ACTION_OUTPUT, SHAPES_GRAPH_IRI,
};
use decision_cli::vocab::IRI_DEC_GRAPH_ORCHESTRATION;

const NS_DEC: &str = "https://decision-cli.dev/ns#";
const FEATURE_CLASS_IRI: &str = "https://decision-cli.dev/ns#Feature";

fn graph_name() -> GraphName {
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

fn typed_quad(subject: &NamedNode, predicate: &str, type_iri: &str) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(predicate),
        NamedNode::new_unchecked(type_iri),
        graph_name(),
    )
}

fn external_origin_quad(subject: &NamedNode, value: &str) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(EXTERNAL_ORIGIN_PROP),
        Literal::new_simple_literal(value),
        graph_name(),
    )
}

fn is_backfill_true_quad(subject: &NamedNode) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNode::new_unchecked(IS_MIGRATION_BACKFILL_PROP),
        Literal::new_typed_literal(
            "true",
            NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean"),
        ),
        graph_name(),
    )
}

/// TC-121: combined positive + negative test for the FT-071 exit
/// criterion. Models the per-type shape composition (FT-072 territory)
/// by directly exercising `:BoundaryArtifactShape`'s Rust-side
/// validator, since the BoundaryArtifact branch is what satisfies the
/// per-type motivational `sh:or` for boundary-originating instances.
#[test]
fn class_satisfies_motivational_or() {
    let handle = OntologyHandle::load().expect("ontology + FT-071 shapes load");

    // ----- (1) Positive: Feature + InitialRequest with external_origin --------
    //
    // A fixture Feature artifact tagged as a BoundaryArtifact subclass
    // (InitialRequest) carrying only the mechanical block (modelled here
    // by its conformance to `:BoundaryArtifactShape`'s sole property —
    // mechanical validation is exercised by FT-069's TC-119) plus the
    // dec:external_origin literal. No motivational predicate edge.
    //
    // The per-type `:FeatureShape` (FT-072) accepts this via its
    // motivational `sh:or`'s first branch `[ sh:class dec:BoundaryArtifact ]`;
    // we model that by validating against `:BoundaryArtifactShape`
    // directly — passing the boundary branch IS the operative claim of
    // FT-071, and `sh:class dec:BoundaryArtifact` with subClassOf
    // reasoning admits the InitialRequest subclass uniformly.

    let feature = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc121-feature");
    let quads = vec![
        typed_quad(
            &feature,
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            FEATURE_CLASS_IRI,
        ),
        typed_quad(
            &feature,
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            INITIAL_REQUEST,
        ),
        external_origin_quad(&feature, "chat-transcript:design-conversation-2026-05-25"),
    ];

    validate_boundary_artifact(&quads, &feature).expect(
        "Feature tagged as :InitialRequest with dec:external_origin must satisfy \
         :BoundaryArtifactShape — proving the boundary branch of the per-type \
         motivational sh:or accepts it without any motivational predicate edge",
    );

    // ----- (2) Negative: same artifact without external_origin ----------------

    let stripped: Vec<Quad> = quads
        .into_iter()
        .filter(|q| q.predicate.as_str() != EXTERNAL_ORIGIN_PROP)
        .collect();
    let err = validate_boundary_artifact(&stripped, &feature).expect_err(
        "Feature tagged as :InitialRequest WITHOUT dec:external_origin must \
         fail :BoundaryArtifactShape validation",
    );
    assert!(
        err.report.contains(EXTERNAL_ORIGIN_PROP) || err.report.contains("external_origin"),
        "violation report must name the dec:external_origin property path; got: {}",
        err.report
    );
    let has_external_origin_path = err
        .violations
        .iter()
        .any(|v| v.path == EXTERNAL_ORIGIN_PROP);
    assert!(
        has_external_origin_path,
        "violation list must include the dec:external_origin path; got: {:?}",
        err.violations
    );

    // ----- (3) MigrationBackfill lacking isMigrationBackfill true -------------

    let backfill = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc121-backfill");
    // Carries external_origin (so :BoundaryArtifactShape passes), but
    // lacks dec:isMigrationBackfill true — :MigrationBackfillShape rejects.
    let backfill_quads = vec![
        typed_quad(
            &backfill,
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            MIGRATION_BACKFILL,
        ),
        external_origin_quad(&backfill, "migration-batch:2026-05-25"),
    ];
    validate_boundary_artifact(&backfill_quads, &backfill)
        .expect("backfill carries valid dec:external_origin");
    let backfill_err = validate_migration_backfill(&backfill_quads, &backfill).expect_err(
        "MigrationBackfill instance lacking dec:isMigrationBackfill true must \
         fail :MigrationBackfillShape extension",
    );
    assert!(
        backfill_err.report.contains(IS_MIGRATION_BACKFILL_PROP)
            || backfill_err.report.contains("isMigrationBackfill"),
        "violation report must name the dec:isMigrationBackfill path; got: {}",
        backfill_err.report
    );

    // And the positive: adding the flag makes it pass.
    let mut backfill_ok_quads = backfill_quads.clone();
    backfill_ok_quads.push(is_backfill_true_quad(&backfill));
    validate_migration_backfill(&backfill_ok_quads, &backfill).expect(
        "MigrationBackfill with dec:isMigrationBackfill true must validate against \
         :MigrationBackfillShape",
    );

    // ----- (4) Four subclasses present as rdfs:subClassOf dec:BoundaryArtifact

    // Single SPARQL query over the bootstrap shapes graph: enumerate
    // every subclass of dec:BoundaryArtifact and assert the slice-1 four
    // are all present. We do NOT enumerate the four in the query — the
    // query asks the graph and we cross-check the returned set.
    let q = format!(
        "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
         SELECT ?sub WHERE {{ GRAPH <{g}> {{ \
             ?sub rdfs:subClassOf <{parent}> \
         }} }}",
        g = SHAPES_GRAPH_IRI,
        parent = BOUNDARY_ARTIFACT_CLASS,
    );
    let results = handle.store().query(q.as_str()).expect("query executes");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions");
    };
    let mut subclasses: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::NamedNode(nn)) = sol.get("sub") {
            subclasses.push(nn.as_str().to_string());
        }
    }
    for required in BOUNDARY_ARTIFACT_SUBCLASSES {
        assert!(
            subclasses.iter().any(|s| s == required),
            "expected <{required}> rdfs:subClassOf <{BOUNDARY_ARTIFACT_CLASS}> in the \
             shapes graph; got subclasses: {subclasses:?}"
        );
    }

    // Belt-and-braces: cross-check the explicit four IRIs by re-listing.
    for s in [
        SENSING_ACTION_OUTPUT,
        INITIAL_REQUEST,
        BOOTSTRAP_ARTIFACT,
        MIGRATION_BACKFILL,
    ] {
        assert!(
            subclasses.iter().any(|x| x == s),
            "missing slice-1 subclass: <{s}>"
        );
        // And confirm the IRI string starts with the expected dec: namespace.
        assert!(s.starts_with(NS_DEC));
    }

    // ----- (5) IRI byte-for-byte parity between Rust constants and TTL --------
    //
    // Cheap drift detector: every IRI we expose as a public constant
    // should appear in the embedded TTL (either as a full IRI or in its
    // prefixed shorthand form).
    let ttl = BOUNDARY_ARTIFACT_SHAPES_TTL;
    let expected = [
        BOUNDARY_ARTIFACT_CLASS,
        BOUNDARY_ARTIFACT_SHAPE,
        MIGRATION_BACKFILL_SHAPE,
        SENSING_ACTION_OUTPUT,
        INITIAL_REQUEST,
        BOOTSTRAP_ARTIFACT,
        MIGRATION_BACKFILL,
        EXTERNAL_ORIGIN_PROP,
        IS_MIGRATION_BACKFILL_PROP,
    ];
    for iri in expected {
        let local_or_prefixed = ttl.contains(iri) || contains_prefixed_form(ttl, iri);
        assert!(
            local_or_prefixed,
            "expected IRI {iri:?} (or its dec:-prefixed shorthand) to appear in the \
             embedded boundary-artifact TTL"
        );
    }
}

/// True if `ttl` contains the prefixed shorthand of `full_iri` for the
/// `dec:` / `xsd:` / `sh:` namespaces. We accept either the full IRI or
/// the shorthand to keep this drift check resilient against stylistic
/// Turtle authoring decisions.
fn contains_prefixed_form(ttl: &str, full_iri: &str) -> bool {
    for (prefix, namespace) in [
        ("dec:", "https://decision-cli.dev/ns#"),
        ("xsd:", "http://www.w3.org/2001/XMLSchema#"),
        ("sh:", "http://www.w3.org/ns/shacl#"),
    ] {
        if let Some(local) = full_iri.strip_prefix(namespace) {
            let prefixed = format!("{prefix}{local}");
            if ttl.contains(&prefixed) {
                return true;
            }
        }
    }
    false
}
