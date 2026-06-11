//! TC-120 — motivational-predicate vocabulary (FT-070 / ADR-038 / ADR-039).
//!
//! Exit criterion for FT-070:
//!
//! 1. Every `rdf:Property` declaration in `motivational-predicates.ttl`
//!    carries `rdfs:subPropertyOf prov:wasDerivedFrom`.
//! 2. The set of predicates in the file matches the slice-1 vocabulary
//!    table (`MOTIVATIONAL_PREDICATES`) — single source of truth fitness
//!    check across TTL and Rust.
//! 3. A fixture graph asserting `:f1 :addresses :feedback1`,
//!    `:f1 :decomposesFrom :brief1`, `:adr1 :decidesFor :f1` produces
//!    three results from a `SELECT ?ancestor WHERE { :f1 prov:wasDerivedFrom* ?ancestor }`
//!    query, without the test author enumerating any motivational
//!    predicate name in the query itself — the materialisation step
//!    discovers them from the loaded shapes graph.
//! 4. The build-time range-agreement fitness check passes for the
//!    slice-1 type shapes: every per-type `sh:property [sh:path :foo ;
//!    sh:class :Bar]` agrees with `dec:foo`'s declared `rdfs:range`
//!    (or `:foo` has no range declared, in which case the per-type
//!    shape pins it). The slice-1 per-type shape catalog is empty
//!    until FT-072 lands; the check is therefore vacuous today, but the
//!    implementation runs against any per-type shape that does exist.

use std::collections::{HashMap, HashSet};

use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::ontology::{
    OntologyHandle, MOTIVATIONAL_PREDICATES, MOTIVATIONAL_PREDICATES_TTL, PROV_WAS_DERIVED_FROM,
    SHAPES_GRAPH_IRI,
};

const NS_DEC: &str = "https://decision-cli.dev/ns#";

/// TC-120: combined positive case for the FT-070 exit criterion. Loads
/// the embedded ontology (which now wires the motivational vocabulary
/// into the shapes graph at parse time), parses the shipped TTL
/// directly to enumerate property declarations, then exercises the
/// fixture walk.
#[test]
fn tc_120_motivational_predicates_declared_subpropertyof_was() {
    let handle = OntologyHandle::load().expect("ontology + FT-070 shapes load");

    // (1) & (2) — every rdf:Property declaration in the file carries
    // rdfs:subPropertyOf prov:wasDerivedFrom AND the set of predicates
    // matches the slice-1 vocabulary table.
    let declared = predicates_declared_in_shape_file();
    let expected: HashSet<String> = MOTIVATIONAL_PREDICATES
        .iter()
        .map(|p| (*p).to_string())
        .collect();
    assert_eq!(
        declared.iter().cloned().collect::<HashSet<_>>(),
        expected,
        "motivational-predicates.ttl rdf:Property declarations must match \
         MOTIVATIONAL_PREDICATES exactly — slice-1 vocabulary table is the \
         single source of truth"
    );
    let subprops = subproperties_of_wasderivedfrom_in_shape_file();
    for pred in &declared {
        assert!(
            subprops.contains(pred),
            "predicate <{pred}> is declared rdf:Property in \
             motivational-predicates.ttl but lacks rdfs:subPropertyOf \
             prov:wasDerivedFrom — every motivational predicate must \
             carry the subProperty assertion (ADR-039)"
        );
    }

    // The ontology loader's invariant check enforces the same property
    // at parse time, against the in-store representation rather than the
    // raw TTL — exercise it indirectly by asserting the handle loaded.
    assert!(handle.shapes_graph().count() > 0);

    // (3) Fixture walk: f1 → feedback1 (via :addresses),
    //                   f1 → brief1    (via :decomposesFrom),
    //                   adr1 → f1      (via :decidesFor — backward edge,
    //                                   doesn't contribute to f1's
    //                                   ancestor set).
    let store = Store::new().expect("fresh store");
    let g_iri = "https://decision-cli.dev/ns/test/ft-070";
    let graph = NamedNode::new_unchecked(g_iri);

    // Load the shape graph so the materialisation step can discover the
    // motivational vocabulary without enumerating predicate names in
    // the test source.
    let shapes_graph = NamedNode::new_unchecked(SHAPES_GRAPH_IRI);
    let parser = oxigraph::io::RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(shapes_graph));
    store
        .load_from_reader(parser, MOTIVATIONAL_PREDICATES_TTL.as_bytes())
        .expect("load motivational-predicates.ttl");

    // Fixture triples (the spec names these three by construction).
    let f1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/f1");
    let feedback1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/feedback1");
    let brief1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/brief1");
    let adr1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/adr1");

    let addresses = NamedNode::new_unchecked(format!("{NS_DEC}addresses"));
    let decomposes_from = NamedNode::new_unchecked(format!("{NS_DEC}decomposesFrom"));
    let decides_for = NamedNode::new_unchecked(format!("{NS_DEC}decidesFor"));

    insert(&store, &f1, &addresses, &feedback1, &graph);
    insert(&store, &f1, &decomposes_from, &brief1, &graph);
    insert(&store, &adr1, &decides_for, &f1, &graph);

    // Materialise prov:wasDerivedFrom edges from every motivational
    // edge in the fixture graph. The materialisation discovers the
    // motivational predicate set from the shapes graph — the SPARQL
    // UPDATE does NOT enumerate predicate names; the WHERE clause
    // filters via the rdfs:subPropertyOf prov:wasDerivedFrom triples
    // loaded above.
    let update = format!(
        "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
         PREFIX prov: <http://www.w3.org/ns/prov#>\n\
         INSERT {{ GRAPH <{g}> {{ ?s prov:wasDerivedFrom ?o }} }}\n\
         WHERE {{\n\
           GRAPH <{g}> {{ ?s ?p ?o }}\n\
           GRAPH <{shapes}> {{ ?p rdfs:subPropertyOf prov:wasDerivedFrom }}\n\
         }}",
        g = g_iri,
        shapes = SHAPES_GRAPH_IRI,
    );
    store
        .update(update.as_str())
        .expect("materialise wasDerivedFrom");

    // The query uses ONLY `prov:wasDerivedFrom*` — no motivational
    // predicate names appear in the query text.
    let query = format!(
        "PREFIX prov: <http://www.w3.org/ns/prov#>\n\
         SELECT ?ancestor WHERE {{ GRAPH <{g}> {{ \
            <https://decision-cli.dev/ns/test/f1> prov:wasDerivedFrom* ?ancestor \
         }} }}",
        g = g_iri,
    );
    let ancestors = collect_ancestors(&store, query.as_str());

    let expected_ancestors: HashSet<String> = [
        "https://decision-cli.dev/ns/test/f1".to_string(),
        "https://decision-cli.dev/ns/test/feedback1".to_string(),
        "https://decision-cli.dev/ns/test/brief1".to_string(),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        ancestors, expected_ancestors,
        "prov:wasDerivedFrom* from :f1 must return three ancestors: \
         {{:f1 (self), :feedback1 (via :addresses), :brief1 (via :decomposesFrom)}}"
    );

    // (4) Range-agreement fitness check on the slice-1 per-type shape
    // catalog. The catalog is empty until FT-072 lands; the check
    // iterates whatever is present and verifies every per-type
    // `sh:property [ sh:path :foo ; sh:class :Bar ]` agrees with
    // `dec:foo`'s declared `rdfs:range` (or `:foo` has no declared
    // range and the per-type shape pins it).
    let pertype_constraints = per_type_class_constraints(handle);
    let ranges = declared_ranges_by_predicate();
    for (path, class) in &pertype_constraints {
        if let Some(declared) = ranges.get(path) {
            assert!(
                declared.contains(class),
                "per-type shape pins <{path}> to sh:class <{class}> but \
                 the declared rdfs:range set for <{path}> in \
                 motivational-predicates.ttl is {declared:?} — \
                 ranges must agree"
            );
        }
        // If no range declared at all, the per-type shape is the only
        // authority — acceptable per FT-070 §Behaviour (range omitted
        // for predicates with multiple legitimate ranges).
    }
}

// --- Helpers ----------------------------------------------------------------

fn insert(store: &Store, s: &NamedNode, p: &NamedNode, o: &NamedNode, g: &NamedNode) {
    let q = Quad::new(s.clone(), p.clone(), o.clone(), g.clone());
    store.insert(&q).expect("insert quad");
}

fn collect_ancestors(store: &Store, query: &str) -> HashSet<String> {
    let results = store.query(query).expect("query executes");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions");
    };
    let mut out = HashSet::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(term) = sol.get("ancestor") {
            if let oxigraph::model::Term::NamedNode(nn) = term {
                out.insert(nn.as_str().to_string());
            }
        }
    }
    out
}

/// Parse `MOTIVATIONAL_PREDICATES_TTL` directly into a fresh store and
/// return the set of IRIs declared as `rdf:Property` in it. We re-parse
/// rather than scrape strings so the assertion is invariant under
/// stylistic changes (prefixed vs full form, attribute order, …).
fn predicates_declared_in_shape_file() -> HashSet<String> {
    let store = parse_shape_file_into_default_graph();
    let q = "PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n\
             PREFIX dec:  <https://decision-cli.dev/ns#>\n\
             SELECT ?p WHERE {\n\
               ?p a rdf:Property .\n\
               FILTER (STRSTARTS(STR(?p), \"https://decision-cli.dev/ns#\"))\n\
             }";
    let results = store.query(q).expect("query");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions");
    };
    let mut out = HashSet::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::NamedNode(nn)) = sol.get("p") {
            out.insert(nn.as_str().to_string());
        }
    }
    out
}

fn subproperties_of_wasderivedfrom_in_shape_file() -> HashSet<String> {
    let store = parse_shape_file_into_default_graph();
    let q = format!(
        "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
         SELECT ?p WHERE {{ ?p rdfs:subPropertyOf <{parent}> }}",
        parent = PROV_WAS_DERIVED_FROM,
    );
    let results = store.query(q.as_str()).expect("query");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions");
    };
    let mut out = HashSet::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::NamedNode(nn)) = sol.get("p") {
            out.insert(nn.as_str().to_string());
        }
    }
    out
}

fn declared_ranges_by_predicate() -> HashMap<String, HashSet<String>> {
    let store = parse_shape_file_into_default_graph();
    let q = "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
             SELECT ?p ?r WHERE { ?p rdfs:range ?r }";
    let results = store.query(q).expect("query");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions");
    };
    let mut out: HashMap<String, HashSet<String>> = HashMap::new();
    for sol in sols {
        let sol = sol.expect("solution");
        let (Some(oxigraph::model::Term::NamedNode(p)), Some(oxigraph::model::Term::NamedNode(r))) =
            (sol.get("p"), sol.get("r"))
        else {
            continue;
        };
        out.entry(p.as_str().to_string())
            .or_default()
            .insert(r.as_str().to_string());
    }
    out
}

fn parse_shape_file_into_default_graph() -> Store {
    let store = Store::new().expect("store");
    let parser = oxigraph::io::RdfParser::from_format(RdfFormat::Turtle);
    store
        .load_from_reader(parser, MOTIVATIONAL_PREDICATES_TTL.as_bytes())
        .expect("parse motivational-predicates.ttl");
    store
}

/// Iterate every `sh:property [ sh:path ?path ; sh:class ?class ]` in the
/// embedded shapes graph (which is where FT-072's per-type shapes will
/// land). Slice-1 catalogue is empty; the helper is still exercised so
/// future per-type additions are checked automatically.
fn per_type_class_constraints(handle: &OntologyHandle) -> Vec<(String, String)> {
    let q = format!(
        "PREFIX sh: <http://www.w3.org/ns/shacl#>\n\
         SELECT ?path ?cls WHERE {{ GRAPH <{g}> {{\n\
           ?shape sh:property ?p .\n\
           ?p sh:path ?path ; sh:class ?cls .\n\
           FILTER (STRSTARTS(STR(?path), \"https://decision-cli.dev/ns#\"))\n\
         }} }}",
        g = SHAPES_GRAPH_IRI,
    );
    let results = handle.store().query(q.as_str()).expect("query");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions");
    };
    let mut out = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        let (
            Some(oxigraph::model::Term::NamedNode(path)),
            Some(oxigraph::model::Term::NamedNode(cls)),
        ) = (sol.get("path"), sol.get("cls"))
        else {
            continue;
        };
        out.push((path.as_str().to_string(), cls.as_str().to_string()));
    }
    out
}

// Suppress an unused-import warning if NamedNodeRef ends up not used
// after iteration.
#[allow(dead_code)]
fn _typecheck(_g: NamedNodeRef<'_>) {}
