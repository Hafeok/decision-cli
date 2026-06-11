//! TC-122 — per-type shape files compose universal blocks (FT-072 / ADR-038).
//!
//! Exit criterion for FT-072: every per-type shape file loads at
//! bootstrap, composes the universal mechanical fragment via `sh:and`,
//! includes the BoundaryArtifact branch as the first `sh:or`
//! alternative (where applicable), and the Rust and Python copies of
//! the shape directory are byte-identical.
//!
//! What this test asserts:
//!   1. `OntologyHandle::load()` succeeds with the full FT-072 catalog
//!      loaded — proves `dec init` (which calls the same loader) parses
//!      every shape file without error.
//!   2. For every per-type shape file, each `sh:NodeShape` whose
//!      `sh:targetClass` lives in the `dec:` namespace composes
//!      `dec:MechanicalProvenanceShape` via `sh:and`.
//!   3. For every per-type shape file NOT in
//!      `PER_TYPE_BOUNDARY_EXEMPT_FILES`, the first member of the
//!      shape's `sh:or` list is `[ a sh:NodeShape ; sh:class
//!      dec:BoundaryArtifact ]`.
//!   4. The Rust source directory
//!      `crates/dec-ontology/src/ontology/assets/shapes/` matches
//!      the Python copy `workers/_shared/shapes/` byte-for-byte (the
//!      dual-validator-agreement fitness check from FT-072 §Behaviour
//!      step 3).
//!   5. The build-time range-agreement check (shared with TC-120)
//!      passes: every per-type `sh:property [sh:path :foo ; sh:class
//!      :Bar]` agrees with `dec:foo`'s declared `rdfs:range` (or `:foo`
//!      has no declared range and the per-type shape pins it).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{BlankNode, GraphName, NamedNode, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use decision_cli::core::ontology::{
    OntologyHandle, MECHANICAL_PROVENANCE_SHAPE, MOTIVATIONAL_PREDICATES_TTL,
    PER_TYPE_BOUNDARY_EXEMPT_FILES, PER_TYPE_SHAPE_FILES, PER_TYPE_SHAPE_IRIS,
};

const NS_DEC: &str = "https://decision-cli.dev/ns#";
const SH_AND: &str = "http://www.w3.org/ns/shacl#and";
const SH_OR: &str = "http://www.w3.org/ns/shacl#or";
const SH_CLASS: &str = "http://www.w3.org/ns/shacl#class";
const SH_PATH: &str = "http://www.w3.org/ns/shacl#path";
const SH_NODE_SHAPE: &str = "http://www.w3.org/ns/shacl#NodeShape";
const SH_PROPERTY: &str = "http://www.w3.org/ns/shacl#property";
const SH_TARGET_CLASS: &str = "http://www.w3.org/ns/shacl#targetClass";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const BOUNDARY_ARTIFACT: &str = "https://decision-cli.dev/ns#BoundaryArtifact";

/// TC-122: the single exit-criterion test for FT-072. Five assertions in
/// one test method matching the runner-args recorded on the TC.
#[test]
fn tc_122_per_type_shape_files_load_and_compose_universal_bl() {
    // ---- (1) Ontology loader parses the full FT-072 catalog ------------
    let _handle = OntologyHandle::load().expect(
        "dec init: ontology + FT-072 per-type shape catalog must load \
         without parse error",
    );

    // ---- Catalog completeness: file lists agree ------------------------
    assert_eq!(
        PER_TYPE_SHAPE_FILES.len(),
        PER_TYPE_SHAPE_IRIS.len(),
        "PER_TYPE_SHAPE_FILES and PER_TYPE_SHAPE_IRIS must be the same length"
    );

    // ---- (2) + (3) Per-file iterator: composition + boundary-first ----
    for (filename, ttl) in PER_TYPE_SHAPE_FILES {
        let store = parse_shape_file(ttl);
        let shapes = node_shapes_with_target_class(&store);
        assert!(
            !shapes.is_empty(),
            "per-type shape file '{filename}' must declare at least one \
             sh:NodeShape with sh:targetClass"
        );
        for (shape_iri, target_iri) in &shapes {
            assert!(
                shape_targets_dec_namespace(target_iri),
                "per-type shape file '{filename}': shape <{shape_iri}> targets \
                 <{target_iri}> which is not in the dec: namespace — every \
                 per-type shape must target a dec: class"
            );
            assert!(
                shape_composes_mechanical_via_and(&store, shape_iri),
                "per-type shape file '{filename}': shape <{shape_iri}> must \
                 compose <{MECHANICAL_PROVENANCE_SHAPE}> via sh:and \
                 (FT-072 / ADR-038 §Behaviour)"
            );

            // BoundaryArtifact branch invariant — non-exempt only.
            if PER_TYPE_BOUNDARY_EXEMPT_FILES.iter().any(|f| f == filename) {
                continue;
            }
            assert!(
                shape_or_first_branch_is_boundary(&store, shape_iri),
                "per-type shape file '{filename}': shape <{shape_iri}> must \
                 declare [ a sh:NodeShape ; sh:class <{BOUNDARY_ARTIFACT}> ] \
                 as the FIRST member of its sh:or list (FT-072 / ADR-040 \
                 boundary-first invariant)"
            );
        }
    }

    // ---- (4) Rust source vs Python copy byte-identical ----------------
    assert_shape_directories_match();

    // ---- (5) Range-agreement check ------------------------------------
    let declared_ranges = declared_ranges_in_motivational_file();
    for (filename, ttl) in PER_TYPE_SHAPE_FILES {
        let store = parse_shape_file(ttl);
        for (path, class) in per_type_class_constraints(&store) {
            if let Some(declared) = declared_ranges.get(&path) {
                assert!(
                    declared.contains(&class),
                    "per-type shape file '{filename}': sh:property \
                     [ sh:path <{path}> ; sh:class <{class}> ] disagrees \
                     with motivational-predicates.ttl declared rdfs:range \
                     set for <{path}> = {declared:?} — range-agreement \
                     fitness check failed (TC-120/TC-122)"
                );
            }
            // No declared range → per-type shape pins it. Allowed per
            // FT-070 §Behaviour for predicates with multiple legitimate
            // ranges.
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_shape_file(ttl: &str) -> Store {
    let store = Store::new().expect("store");
    let parser = RdfParser::from_format(RdfFormat::Turtle);
    store
        .load_from_reader(parser, ttl.as_bytes())
        .expect("per-type shape file must parse as Turtle");
    store
}

/// Returns `(shape_iri, target_class_iri)` for every `sh:NodeShape` in
/// the store that carries a `sh:targetClass`.
fn node_shapes_with_target_class(store: &Store) -> Vec<(String, String)> {
    let q = format!(
        "SELECT ?shape ?cls WHERE {{ \
            ?shape a <{shape_cls}> ; <{tc}> ?cls . \
         }}",
        shape_cls = SH_NODE_SHAPE,
        tc = SH_TARGET_CLASS
    );
    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("query") else {
        panic!("solutions");
    };
    for sol in sols {
        let sol = sol.expect("sol");
        let (Some(Term::NamedNode(s)), Some(Term::NamedNode(c))) =
            (sol.get("shape"), sol.get("cls"))
        else {
            continue;
        };
        out.push((s.as_str().to_string(), c.as_str().to_string()));
    }
    out
}

fn shape_targets_dec_namespace(target_iri: &str) -> bool {
    target_iri.starts_with(NS_DEC)
}

/// True if the shape carries `sh:and ( ... :MechanicalProvenanceShape ... )`.
fn shape_composes_mechanical_via_and(store: &Store, shape_iri: &str) -> bool {
    let q = format!(
        "PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n\
         ASK {{ \
            <{shape}> <{sh_and}> ?lst . \
            ?lst rdf:rest* ?node . \
            ?node rdf:first <{mech}> . \
         }}",
        shape = shape_iri,
        sh_and = SH_AND,
        mech = MECHANICAL_PROVENANCE_SHAPE,
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

/// True if the shape's `sh:or` list FIRST member matches
/// `[ a sh:NodeShape ; sh:class dec:BoundaryArtifact ]`.
fn shape_or_first_branch_is_boundary(store: &Store, shape_iri: &str) -> bool {
    let q = format!(
        "PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n\
         SELECT ?branch WHERE {{ \
            <{shape}> <{sh_or}> ?lst . \
            ?lst rdf:first ?branch . \
         }}",
        shape = shape_iri,
        sh_or = SH_OR,
    );
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("query") else {
        return false;
    };
    let Some(sol) = sols.into_iter().next() else {
        return false;
    };
    let Ok(sol) = sol else {
        return false;
    };
    let Some(branch) = sol.get("branch") else {
        return false;
    };

    // Branch should be a blank node carrying:
    //   - rdf:type sh:NodeShape
    //   - sh:class dec:BoundaryArtifact
    let branch_node: Subject = match branch {
        Term::NamedNode(n) => n.clone().into(),
        Term::BlankNode(b) => b.clone().into(),
        _ => return false,
    };

    let has_node_shape = ask_subject_pred_obj(store, &branch_node, RDF_TYPE, SH_NODE_SHAPE);
    let has_class = ask_subject_pred_obj(store, &branch_node, SH_CLASS, BOUNDARY_ARTIFACT);
    has_node_shape && has_class
}

fn ask_subject_pred_obj(store: &Store, subj: &Subject, pred: &str, obj: &str) -> bool {
    let pattern_subj = match subj {
        Subject::NamedNode(n) => format!("<{}>", n.as_str()),
        Subject::BlankNode(b) => format!("_:{}", b.as_str()),
        _ => return false,
    };
    let q = format!(
        "ASK {{ {subj} <{pred}> <{obj}> }}",
        subj = pattern_subj,
        pred = pred,
        obj = obj,
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

/// Iterates `sh:property [ sh:path ?p ; sh:class ?c ]` in the given
/// shape store. Filters to dec: paths to keep the check focused on
/// motivational predicates.
fn per_type_class_constraints(store: &Store) -> Vec<(String, String)> {
    let q = format!(
        "SELECT ?path ?cls WHERE {{ \
           ?shape <{sh_prop}> ?p . \
           ?p <{sh_path}> ?path ; <{sh_class}> ?cls . \
           FILTER (STRSTARTS(STR(?path), \"{ns}\")) \
         }}",
        sh_prop = SH_PROPERTY,
        sh_path = SH_PATH,
        sh_class = SH_CLASS,
        ns = NS_DEC,
    );
    let mut out = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str()).expect("query") else {
        return out;
    };
    for sol in sols {
        let sol = sol.expect("sol");
        let (Some(Term::NamedNode(p)), Some(Term::NamedNode(c))) =
            (sol.get("path"), sol.get("cls"))
        else {
            continue;
        };
        out.push((p.as_str().to_string(), c.as_str().to_string()));
    }
    out
}

fn declared_ranges_in_motivational_file() -> HashMap<String, HashSet<String>> {
    let store = Store::new().expect("store");
    let parser = RdfParser::from_format(RdfFormat::Turtle);
    store
        .load_from_reader(parser, MOTIVATIONAL_PREDICATES_TTL.as_bytes())
        .expect("parse motivational-predicates.ttl");
    let q = "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
             SELECT ?p ?r WHERE { ?p rdfs:range ?r }";
    let QueryResults::Solutions(sols) = store.query(q).expect("query") else {
        return HashMap::new();
    };
    let mut out: HashMap<String, HashSet<String>> = HashMap::new();
    for sol in sols {
        let sol = sol.expect("sol");
        let (Some(Term::NamedNode(p)), Some(Term::NamedNode(r))) = (sol.get("p"), sol.get("r"))
        else {
            continue;
        };
        out.entry(p.as_str().to_string())
            .or_default()
            .insert(r.as_str().to_string());
    }
    out
}

fn assert_shape_directories_match() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/decision-cli/ → repo root
    let repo_root = manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root from CARGO_MANIFEST_DIR")
        .to_path_buf();
    let rust_shapes = repo_root.join("crates/dec-ontology/src/ontology/assets/shapes");
    let py_shapes = repo_root.join("workers/_shared/shapes");

    assert!(
        rust_shapes.is_dir(),
        "Rust shapes directory missing: {}",
        rust_shapes.display()
    );
    assert!(
        py_shapes.is_dir(),
        "Python shapes directory missing: {}",
        py_shapes.display()
    );

    let rust_files = collect_ttl_files(&rust_shapes);
    let py_files = collect_ttl_files(&py_shapes);

    assert_eq!(
        rust_files,
        py_files,
        "Rust shapes directory and Python copy must contain identical file \
         sets (FT-072 byte-identical invariant). \
         Rust-only: {:?}, Python-only: {:?}",
        rust_files.difference(&py_files).collect::<Vec<_>>(),
        py_files.difference(&rust_files).collect::<Vec<_>>(),
    );

    for name in &rust_files {
        let r = fs::read(rust_shapes.join(name)).expect("read rust");
        let p = fs::read(py_shapes.join(name)).expect("read python");
        assert_eq!(
            r, p,
            "shape file '{name}' differs between Rust source and Python copy \
             — FT-072 §Behaviour step 3 requires byte-identical content"
        );
    }
}

fn collect_ttl_files(dir: &PathBuf) -> HashSet<String> {
    let mut out = HashSet::new();
    for entry in fs::read_dir(dir).expect("read_dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".ttl") {
            out.insert(name);
        }
    }
    out
}

// Suppress unused-imports if a future refactor drops the blank-node
// path. BlankNode + GraphName + NamedNode appear in ask_subject_pred_obj
// indirectly through their Subject impls.
#[allow(dead_code)]
fn _typecheck(_g: GraphName, _b: BlankNode, _n: NamedNode) {}
