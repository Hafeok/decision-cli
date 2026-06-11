//! TC-141 — Brief artifact type is wired into the catalog (FT-076 / ADR-044).
//!
//! Exit criterion for FT-076:
//!
//! 1. `dec:Brief` and `dec:Acknowledgement` are declared as `rdfs:Class` in
//!    the embedded ontology — without those class declarations, the per-type
//!    shape's `sh:targetClass dec:Brief` and the `sh:class dec:Acknowledgement`
//!    range constraint on `dec:acknowledges` have nothing to bind to.
//! 2. The Brief body-field properties (`title`, `premise`, `goal`,
//!    `successCriteria`) and the Brief-side edges (`decomposesInto`,
//!    `excludes`, `acknowledges`, `references`) are declared as
//!    `rdf:Property` with the right `rdfs:domain` and (where applicable)
//!    `rdfs:range`.
//! 3. The per-type catalog (`PER_TYPE_SHAPE_FILES`) carries the Brief shape
//!    file, and `dec:BriefShape` declares `sh:property` blocks for each of
//!    the four required body fields and each of the four Brief-side forward
//!    edges.
//! 4. The slice-1 SHACL validator table (`slice1_type_shapes`) lists Brief
//!    with `respondsTo` as its motivational predicate and `accepts_boundary`
//!    set — Briefs are first-class participants in the dual-provenance
//!    discipline (ADR-038) AND can satisfy validation via boundary-artifact
//!    class membership (ADR-040).
//! 5. The motivational vocabulary (`MOTIVATIONAL_PREDICATES_TTL`) declares
//!    `dec:decomposesFrom` with `rdfs:range dec:Brief` — that is the
//!    Feature→Brief motivational edge complemented by the Brief→Feature
//!    `dec:decomposesInto` forward edge added in this feature.
//! 6. A fixture Brief artifact carrying the four required body fields plus
//!    typing as a `dec:BoundaryArtifact` (the boundary-origin case that
//!    most Briefs land as per ADR-040) is structurally well-formed: the
//!    forward edges `decomposesInto`, `excludes`, `acknowledges`,
//!    `references` round-trip through a fresh store and can be queried.

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::graph::shacl::slice1_type_shapes;
use decision_cli::core::ontology::{
    OntologyHandle, MOTIVATIONAL_PREDICATES_TTL, PER_TYPE_SHAPE_FILES, PER_TYPE_SHAPE_IRIS,
};

const NS_DEC: &str = "https://decision-cli.dev/ns#";

#[test]
fn tc_141_pipeline_worker_sdk_add_the_brief_artifact_type_to_product_cli_catalog() {
    let handle = OntologyHandle::load().expect("ontology + FT-076 shapes load");

    // ---- (1) Brief and Acknowledgement are declared classes -------------
    let brief_cls = format!("{NS_DEC}Brief");
    let ack_cls = format!("{NS_DEC}Acknowledgement");
    let feature_cls = format!("{NS_DEC}Feature");
    assert!(
        handle.declares_class(&brief_cls),
        "ontology must declare <{brief_cls}> as rdfs:Class (FT-076)"
    );
    assert!(
        handle.declares_class(&ack_cls),
        "ontology must declare <{ack_cls}> as rdfs:Class (FT-076)"
    );
    assert!(
        handle.declares_class(&feature_cls),
        "ontology must declare <{feature_cls}> as rdfs:Class — Brief's \
         decomposesInto/excludes edges have Feature as their range (FT-076)"
    );

    // ---- (2) Body fields + edges are declared properties ----------------
    for prop_local in [
        "title",
        "premise",
        "goal",
        "successCriteria",
        "decomposesInto",
        "excludes",
        "acknowledges",
        "references",
    ] {
        let iri = format!("{NS_DEC}{prop_local}");
        assert!(
            handle.declares_property(&iri),
            "ontology must declare <{iri}> as rdf:Property (FT-076 §Scope)"
        );
    }

    // Domain/range cross-check on the Brief-side edges.
    assert_property_domain_and_range(
        handle.store(),
        &format!("{NS_DEC}decomposesInto"),
        Some(&brief_cls),
        Some(&feature_cls),
    );
    assert_property_domain_and_range(
        handle.store(),
        &format!("{NS_DEC}excludes"),
        Some(&brief_cls),
        Some(&feature_cls),
    );
    assert_property_domain_and_range(
        handle.store(),
        &format!("{NS_DEC}acknowledges"),
        Some(&brief_cls),
        Some(&ack_cls),
    );

    // ---- (3) Per-type catalog carries Brief shape file ------------------
    let brief_shape_iri = "https://decision-cli.dev/ns#BriefShape";
    assert!(
        PER_TYPE_SHAPE_FILES
            .iter()
            .any(|(name, _)| *name == "brief.ttl"),
        "PER_TYPE_SHAPE_FILES must list brief.ttl (FT-076 / FT-072)"
    );
    assert!(
        PER_TYPE_SHAPE_IRIS
            .iter()
            .any(|(name, iri)| *name == "brief.ttl" && *iri == brief_shape_iri),
        "PER_TYPE_SHAPE_IRIS must map brief.ttl → <{brief_shape_iri}>"
    );
    assert!(
        node_shape_present_in_shapes_graph(handle.store(), brief_shape_iri),
        "<{brief_shape_iri}> must be present in the loaded shapes graph"
    );
    // BriefShape declares sh:property for each required body field + edge.
    let property_paths = shape_property_paths(handle.store(), brief_shape_iri);
    for required_path in [
        "title",
        "premise",
        "goal",
        "successCriteria",
        "decomposesInto",
        "excludes",
        "acknowledges",
        "references",
    ] {
        let iri = format!("{NS_DEC}{required_path}");
        assert!(
            property_paths.contains(&iri),
            "dec:BriefShape must declare sh:property [ sh:path <{iri}> ] (FT-076)"
        );
    }

    // ---- (4) slice1_type_shapes table includes Brief --------------------
    let table = slice1_type_shapes();
    let brief_entry = table
        .get(&brief_cls)
        .expect("slice1_type_shapes must contain dec:Brief");
    assert!(
        brief_entry.accepts_boundary,
        "Brief entry must set accepts_boundary=true (ADR-040)"
    );
    assert!(
        !brief_entry.motivational_exempt,
        "Brief entry must NOT be motivational-exempt — Brief carries respondsTo"
    );
    assert!(
        brief_entry
            .motivational
            .contains(&format!("{NS_DEC}respondsTo")),
        "Brief entry must list dec:respondsTo in its motivational set; got {:?}",
        brief_entry.motivational
    );

    // ---- (5) Motivational vocabulary declares Feature→Brief edge --------
    let store = parse_motivational_ttl();
    let q = format!(
        "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
         ASK {{ <{ns}decomposesFrom> rdfs:range <{ns}Brief> }}",
        ns = NS_DEC,
    );
    assert!(
        matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true))),
        "motivational-predicates.ttl must declare dec:decomposesFrom \
         rdfs:range dec:Brief (FT-070 vocabulary backing FT-076)"
    );

    // ---- (6) Fixture Brief round-trips through a fresh store ------------
    let store = Store::new().expect("fresh store");
    let g = GraphName::DefaultGraph;
    let brief_iri = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc141-brief1");
    let feature_a = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc141-FT-A");
    let feature_b = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc141-FT-B");
    let ack_iri = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc141-ack1");
    let ref_iri = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc141-ref1");

    let dec_brief = NamedNode::new_unchecked(&brief_cls);
    let dec_ack = NamedNode::new_unchecked(&ack_cls);
    let dec_feature = NamedNode::new_unchecked(&feature_cls);
    let dec_boundary = NamedNode::new_unchecked(&format!("{NS_DEC}BoundaryArtifact"));
    let rdf_type = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");

    let p_title = NamedNode::new_unchecked(&format!("{NS_DEC}title"));
    let p_premise = NamedNode::new_unchecked(&format!("{NS_DEC}premise"));
    let p_goal = NamedNode::new_unchecked(&format!("{NS_DEC}goal"));
    let p_success = NamedNode::new_unchecked(&format!("{NS_DEC}successCriteria"));
    let p_decomp_into = NamedNode::new_unchecked(&format!("{NS_DEC}decomposesInto"));
    let p_excludes = NamedNode::new_unchecked(&format!("{NS_DEC}excludes"));
    let p_acks = NamedNode::new_unchecked(&format!("{NS_DEC}acknowledges"));
    let p_refs = NamedNode::new_unchecked(&format!("{NS_DEC}references"));
    let p_external = NamedNode::new_unchecked(&format!("{NS_DEC}external_origin"));

    let quads = vec![
        Quad::new(brief_iri.clone(), rdf_type.clone(), dec_brief, g.clone()),
        // Boundary-origin Briefs (the common case per ADR-040) carry the
        // BoundaryArtifact class membership AND dec:external_origin.
        Quad::new(brief_iri.clone(), rdf_type.clone(), dec_boundary, g.clone()),
        Quad::new(
            brief_iri.clone(),
            p_external,
            Literal::new_simple_literal("chat-transcript:tc-141-fixture"),
            g.clone(),
        ),
        // Body fields.
        Quad::new(
            brief_iri.clone(),
            p_title,
            Literal::new_simple_literal("pipeline-worker-slice-1"),
            g.clone(),
        ),
        Quad::new(
            brief_iri.clone(),
            p_premise,
            Literal::new_simple_literal(
                "Workers are stateless bundle-in / artifact-out functions.",
            ),
            g.clone(),
        ),
        Quad::new(
            brief_iri.clone(),
            p_goal,
            Literal::new_simple_literal("Ship an SDK that workers can consume."),
            g.clone(),
        ),
        Quad::new(
            brief_iri.clone(),
            p_success,
            Literal::new_simple_literal(
                "A worker compiled against the SDK can subscribe, complete, and report.",
            ),
            g.clone(),
        ),
        // Forward edges (Brief → Feature[], Brief → Acknowledgement[], …).
        Quad::new(
            feature_a.clone(),
            rdf_type.clone(),
            dec_feature.clone(),
            g.clone(),
        ),
        Quad::new(feature_b.clone(), rdf_type.clone(), dec_feature, g.clone()),
        Quad::new(ack_iri.clone(), rdf_type, dec_ack, g.clone()),
        Quad::new(
            brief_iri.clone(),
            p_decomp_into,
            feature_a.clone(),
            g.clone(),
        ),
        Quad::new(brief_iri.clone(), p_excludes, feature_b.clone(), g.clone()),
        Quad::new(brief_iri.clone(), p_acks, ack_iri.clone(), g.clone()),
        Quad::new(brief_iri.clone(), p_refs, ref_iri.clone(), g.clone()),
    ];

    store
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .expect("seed Brief fixture");

    // Brief-aware query: "show me all Features decomposed from BRIEF-X".
    let decomp_query = format!(
        "SELECT ?f WHERE {{ <{brief}> <{ns}decomposesInto> ?f }}",
        brief = brief_iri.as_str(),
        ns = NS_DEC,
    );
    let features = collect_named_nodes(&store, &decomp_query, "f");
    assert_eq!(
        features.len(),
        1,
        "Brief must decompose into exactly one Feature in this fixture; got {features:?}"
    );
    assert!(features.contains(&feature_a.as_str().to_string()));

    // Brief-aware query: "show me everything BRIEF-X excluded".
    let exclude_query = format!(
        "SELECT ?f WHERE {{ <{brief}> <{ns}excludes> ?f }}",
        brief = brief_iri.as_str(),
        ns = NS_DEC,
    );
    let excluded = collect_named_nodes(&store, &exclude_query, "f");
    assert_eq!(
        excluded.len(),
        1,
        "Brief must exclude exactly one Feature in this fixture; got {excluded:?}"
    );
    assert!(excluded.contains(&feature_b.as_str().to_string()));

    // Field round-trip on the four required body fields.
    let fields_query = format!(
        "SELECT ?t ?p ?g ?s WHERE {{ \
            <{brief}> <{ns}title>            ?t ; \
                      <{ns}premise>          ?p ; \
                      <{ns}goal>             ?g ; \
                      <{ns}successCriteria>  ?s . \
         }}",
        brief = brief_iri.as_str(),
        ns = NS_DEC,
    );
    let QueryResults::Solutions(sols) = store.query(fields_query.as_str()).expect("query") else {
        panic!("expected solutions");
    };
    let count = sols.into_iter().count();
    assert_eq!(
        count, 1,
        "Brief fixture must carry exactly one title/premise/goal/successCriteria \
         tuple — got {count}"
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_motivational_ttl() -> Store {
    let store = Store::new().expect("store");
    let parser = oxigraph::io::RdfParser::from_format(oxigraph::io::RdfFormat::Turtle);
    store
        .load_from_reader(parser, MOTIVATIONAL_PREDICATES_TTL.as_bytes())
        .expect("parse motivational-predicates.ttl");
    store
}

fn node_shape_present_in_shapes_graph(store: &Store, shape_iri: &str) -> bool {
    let q =
        format!("ASK {{ GRAPH ?g {{ <{shape_iri}> a <http://www.w3.org/ns/shacl#NodeShape> }} }}");
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

/// Enumerate the sh:path values reachable from a shape's sh:property blocks.
fn shape_property_paths(store: &Store, shape_iri: &str) -> std::collections::HashSet<String> {
    let q = format!(
        "PREFIX sh: <http://www.w3.org/ns/shacl#>\n\
         SELECT ?path WHERE {{ GRAPH ?g {{ \
            <{shape_iri}> sh:property ?p . \
            ?p sh:path ?path . \
         }} }}",
    );
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("query") else {
        return std::collections::HashSet::new();
    };
    let mut out = std::collections::HashSet::new();
    for sol in sols {
        let sol = sol.expect("sol");
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("path") {
            out.insert(n.as_str().to_string());
        }
    }
    out
}

fn assert_property_domain_and_range(
    store: &Store,
    prop_iri: &str,
    expected_domain: Option<&str>,
    expected_range: Option<&str>,
) {
    if let Some(domain) = expected_domain {
        let q = format!(
            "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
             ASK {{ GRAPH ?g {{ <{prop_iri}> rdfs:domain <{domain}> }} }}",
        );
        assert!(
            matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true))),
            "<{prop_iri}> must declare rdfs:domain <{domain}> (FT-076)"
        );
    }
    if let Some(range) = expected_range {
        let q = format!(
            "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
             ASK {{ GRAPH ?g {{ <{prop_iri}> rdfs:range <{range}> }} }}",
        );
        assert!(
            matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true))),
            "<{prop_iri}> must declare rdfs:range <{range}> (FT-076)"
        );
    }
}

fn collect_named_nodes(store: &Store, query: &str, var: &str) -> Vec<String> {
    let QueryResults::Solutions(sols) = store.query(query).expect("query") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for sol in sols {
        let sol = sol.expect("sol");
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get(var) {
            out.push(n.as_str().to_string());
        }
    }
    out
}
