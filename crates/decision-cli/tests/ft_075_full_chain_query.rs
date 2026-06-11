//! TC-125 — Full-chain QueryTemplate exit criterion (FT-075 / ADR-043).
//!
//! End-to-end: `dec init` seeds the QueryTemplate catalog, fetches the
//! canonical backward / forward instances, executes them against a
//! fixture provenance chain seeded into the store, and confirms the
//! traversal reaches the expected terminal BoundaryArtifact.
//!
//! Also exercises `dec query template list` / `dec query template show`
//! by invoking the CLI-layer helpers directly so the inspection surface
//! is validated alongside the SPARQL execution path.

use std::env;
use std::fs;
use std::path::PathBuf;

use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::queries::{
    bootstrap_query_template_quads, execute_template, fetch_query_template, list_query_templates,
    FULL_CHAIN_BACKWARD_ID, FULL_CHAIN_FORWARD_ID,
};
use decision_cli::core::store::{orchestration_dump_path, persist_store};
use decision_cli::init::{run as init_run, DefinitionSource};

const NS_DEC: &str = "https://decision-cli.dev/ns#";

fn tempdir(label: &str) -> PathBuf {
    let mut p = env::temp_dir();
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("{label}-{pid}-{nonce}"));
    fs::create_dir_all(&p).expect("create tempdir");
    p
}

fn load_store_from_dump(workdir: &PathBuf) -> Store {
    let dump = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump).expect("read store dump");
    let store = Store::new().expect("store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load store");
    store
}

fn seed_fixture_chain(store: &Store) {
    // Build the FT-075 / TC-125 provenance chain:
    //
    // motivational: focal -addresses-> feedback1 -observedIn-> session1
    //                                                          -used-> brief1
    // mechanical:   focal -wasGeneratedBy-> sessionFocal -used-> brief1
    //
    // brief1 is a Brief and a BoundaryArtifact subclass (InitialRequest),
    // making it the terminal node both traversal branches must reach.
    let g = GraphName::DefaultGraph;
    let focal = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc125-focal");
    let feedback1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc125-feedback1");
    let session1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc125-session1");
    let brief1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc125-brief1");
    let session_focal =
        NamedNode::new_unchecked("https://decision-cli.dev/ns/test/tc125-sessionFocal");

    let dec_brief = NamedNode::new_unchecked(&format!("{NS_DEC}Brief"));
    let dec_feature = NamedNode::new_unchecked(&format!("{NS_DEC}Feature"));
    let dec_initial = NamedNode::new_unchecked(&format!("{NS_DEC}InitialRequest"));
    let dec_external_origin = NamedNode::new_unchecked(&format!("{NS_DEC}external_origin"));
    let dec_addresses = NamedNode::new_unchecked(&format!("{NS_DEC}addresses"));
    let dec_observed_in = NamedNode::new_unchecked(&format!("{NS_DEC}observedIn"));
    let prov_used = NamedNode::new_unchecked("http://www.w3.org/ns/prov#used");
    let prov_gen = NamedNode::new_unchecked("http://www.w3.org/ns/prov#wasGeneratedBy");
    let rdf_type = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");

    let quads = vec![
        // focal carries an rdf:type so the BGP `?ancestor rdf:type ?_t`
        // matches it when it appears in either traversal direction.
        Quad::new(
            focal.clone(),
            rdf_type.clone(),
            dec_feature.clone(),
            g.clone(),
        ),
        // Motivational chain
        Quad::new(
            focal.clone(),
            dec_addresses.clone(),
            feedback1.clone(),
            g.clone(),
        ),
        Quad::new(
            feedback1.clone(),
            dec_observed_in.clone(),
            session1.clone(),
            g.clone(),
        ),
        Quad::new(
            session1.clone(),
            prov_used.clone(),
            brief1.clone(),
            g.clone(),
        ),
        // Mechanical chain
        Quad::new(
            focal.clone(),
            prov_gen.clone(),
            session_focal.clone(),
            g.clone(),
        ),
        Quad::new(
            session_focal.clone(),
            prov_used.clone(),
            brief1.clone(),
            g.clone(),
        ),
        // brief1 typing: Brief + InitialRequest (a BoundaryArtifact subclass)
        Quad::new(
            brief1.clone(),
            rdf_type.clone(),
            dec_brief.clone(),
            g.clone(),
        ),
        Quad::new(
            brief1.clone(),
            rdf_type.clone(),
            dec_initial.clone(),
            g.clone(),
        ),
        // BoundaryArtifact membership obliges dec:external_origin per ADR-040
        Quad::new(
            brief1.clone(),
            dec_external_origin.clone(),
            Literal::new_simple_literal("chat-transcript:tc-125-fixture"),
            g.clone(),
        ),
    ];
    store
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .expect("seed fixture chain");
}

/// TC-125 headline assertion (matches the runner-args function name).
#[test]
fn backward_returns_terminal_boundary_artifacts() {
    // (1) `dec init` succeeds; the QueryTemplate catalog is seeded.
    let workdir = tempdir("ft-075-tc-125");
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init template");

    let store = load_store_from_dump(&workdir);

    // (2) Helper `store.fetch_query_template("qt:full-chain-backward-v1")`
    //     succeeds and returns the canonical SPARQL spec + version.
    let backward =
        fetch_query_template(&store, FULL_CHAIN_BACKWARD_ID).expect("fetch backward template");
    assert_eq!(backward.id, FULL_CHAIN_BACKWARD_ID);
    assert_eq!(backward.version, "1.0.0");
    assert_eq!(backward.language, "SPARQL-1.1");
    assert!(
        backward.spec.contains("prov:wasGeneratedBy"),
        "backward spec must walk mechanical lineage"
    );
    assert!(
        backward.spec.contains("prov:wasDerivedFrom"),
        "backward spec must walk motivational lineage (subPropertyOf parent)"
    );

    // (3) The forward template is also seeded and queryable.
    let forward =
        fetch_query_template(&store, FULL_CHAIN_FORWARD_ID).expect("fetch forward template");
    assert_eq!(forward.id, FULL_CHAIN_FORWARD_ID);
    assert_eq!(forward.version, "1.0.0");
    assert_eq!(forward.language, "SPARQL-1.1");

    // (4) Seed the fixture provenance chain and re-persist; re-load so
    // the test exercises the same path a real consumer would: open the
    // orchestration dump, fetch the template, execute against the store.
    seed_fixture_chain(&store);
    persist_store(&store, &orchestration_dump_path(&workdir)).expect("persist store");
    let store = load_store_from_dump(&workdir);

    // (5) Executing the backward template with ?focal := :tc125-focal
    //     returns brief1 as a terminal ancestor on at least one branch.
    let focal_iri = "https://decision-cli.dev/ns/test/tc125-focal";
    let brief1_iri = "https://decision-cli.dev/ns/test/tc125-brief1";
    let dec_brief = format!("{NS_DEC}Brief");

    let results =
        execute_template(&store, &backward, &[("focal", focal_iri)]).expect("execute backward");
    let rows = collect_rows(results, "ancestor", "ancestor_type", "via");
    let brief_rows: Vec<&Row> = rows.iter().filter(|r| r.subject == brief1_iri).collect();
    assert!(
        !brief_rows.is_empty(),
        "backward traversal from focal must surface brief1 as terminal ancestor; got {rows:#?}"
    );
    let types_seen: Vec<&str> = brief_rows.iter().map(|r| r.kind.as_str()).collect();
    assert!(
        types_seen.contains(&dec_brief.as_str()),
        "backward traversal must report brief1's rdf:type dec:Brief; got types {types_seen:?}"
    );
    let vias_seen: Vec<&str> = brief_rows.iter().map(|r| r.via.as_str()).collect();
    assert!(
        vias_seen.contains(&"mechanical") || vias_seen.contains(&"motivational"),
        "backward traversal must classify the path as either mechanical or motivational; got {vias_seen:?}"
    );

    // (6) Forward template from brief1 reaches focal (or its closer descendants).
    let results_fwd =
        execute_template(&store, &forward, &[("focal", brief1_iri)]).expect("execute forward");
    let rows_fwd = collect_rows(results_fwd, "descendant", "descendant_type", "via");
    let saw_focal_descendant = rows_fwd.iter().any(|r| r.subject == focal_iri);
    assert!(
        saw_focal_descendant,
        "forward traversal from brief1 must surface focal as a descendant; got {rows_fwd:#?}"
    );

    // (7) `dec query template list` shows both templates.
    let listed = list_query_templates(&store).expect("list templates");
    let ids: Vec<&str> = listed.iter().map(|t| t.id.as_str()).collect();
    assert!(
        ids.contains(&FULL_CHAIN_BACKWARD_ID),
        "list_query_templates must include backward; got {ids:?}"
    );
    assert!(
        ids.contains(&FULL_CHAIN_FORWARD_ID),
        "list_query_templates must include forward; got {ids:?}"
    );
}

#[derive(Debug)]
struct Row {
    subject: String,
    kind: String,
    via: String,
}

fn collect_rows(results: QueryResults, subj_var: &str, kind_var: &str, via_var: &str) -> Vec<Row> {
    let QueryResults::Solutions(sols) = results else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        let subject = match sol.get(subj_var) {
            Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
            _ => continue,
        };
        let kind = match sol.get(kind_var) {
            Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
            _ => String::new(),
        };
        let via = match sol.get(via_var) {
            Some(oxigraph::model::Term::Literal(l)) => l.value().to_string(),
            _ => String::new(),
        };
        out.push(Row { subject, kind, via });
    }
    out
}

/// Belt-and-braces: bootstrap_query_template_quads is non-empty and
/// produces quads typed as both `dec:QueryTemplate` and `dec:BoundaryArtifact`
/// so the per-type shape's `sh:or` first branch satisfies post-bootstrap.
#[test]
fn bootstrap_quads_carry_query_template_and_boundary_typing() {
    let quads = bootstrap_query_template_quads();
    assert!(
        !quads.is_empty(),
        "bootstrap_query_template_quads returns at least one quad"
    );
    let rdf_type = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    let qt_class = format!("{NS_DEC}QueryTemplate");
    let boundary = format!("{NS_DEC}BoundaryArtifact");
    let bootstrap = format!("{NS_DEC}BootstrapArtifact");
    let backward_iri = "https://decision-cli.dev/ns/qt/full-chain-backward-v1";
    let mut saw_qt = false;
    let mut saw_boundary_or_bootstrap = false;
    for q in &quads {
        if q.subject.to_string().trim_matches('<').trim_matches('>') == backward_iri
            && q.predicate.as_str() == rdf_type
        {
            let obj = q.object.to_string();
            let trimmed = obj.trim_matches('<').trim_matches('>');
            if trimmed == qt_class {
                saw_qt = true;
            }
            if trimmed == boundary || trimmed == bootstrap {
                saw_boundary_or_bootstrap = true;
            }
        }
    }
    assert!(
        saw_qt,
        "backward template instance must declare rdf:type dec:QueryTemplate"
    );
    assert!(
        saw_boundary_or_bootstrap,
        "backward template instance must declare rdf:type dec:BoundaryArtifact \
         (or BootstrapArtifact subclass) for ADR-040 satisfaction"
    );
}
