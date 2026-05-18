//! `dec init` validation pipeline (FT-008 / ADR-006).
//!
//! FT-006 owns the ontology + shapes, but the test criteria the
//! pipeline gates on (TC-001, TC-003) exercise the whole init flow.
//! This module implements the minimal pipeline needed to make those
//! TCs pass:
//!
//! ```text
//!   parse → SHACL-validate → resolve → cross-validate → persist → provenance
//! ```
//!
//! Each step is a separate function so FT-008 / FT-009 can swap in a
//! richer implementation without touching the CLI plumbing.
//!
//! Failure modes are structured via [`InitError`] so the CLI can name
//! exactly which field / IRI / goal was wrong (ADR-006).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{
    GraphName, GraphNameRef, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term,
};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::bundled;
use crate::ontology::{OntologyError, OntologyHandle};

/// IRI of the bootstrap session record (`dec:session/init-001`).
pub const BOOTSTRAP_SESSION_IRI: &str = "https://decision-cli.dev/ns/session/init-001";

/// Named graph used to hold the persisted orchestration state on init.
pub const ORCHESTRATION_GRAPH_IRI: &str = "https://decision-cli.dev/ns/orchestration";

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
const PROV_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
const PROV_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";
const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";

const DEC_NAME: &str = "https://decision-cli.dev/ns#name";
const DEC_TITLE: &str = "https://decision-cli.dev/ns#title";
const DEC_DESCRIPTION: &str = "https://decision-cli.dev/ns#description";
const DEC_TERMINAL_VALUE_ACTION: &str = "https://decision-cli.dev/ns#terminalValueAction";
const DEC_AUTHORIZED_GOALS: &str = "https://decision-cli.dev/ns#authorizedGoals";
const DEC_COMPATIBLE_GOALS: &str = "https://decision-cli.dev/ns#compatibleGoals";
const DEC_DEFINITION_SOURCE: &str = "https://decision-cli.dev/ns#definitionSource";
const DEC_DEFINITION_HASH: &str = "https://decision-cli.dev/ns#definitionHash";
const DEC_ONTOLOGY_VERSION: &str = "https://decision-cli.dev/ns#ontologyVersion";
const DEC_DEFINITION_FORM: &str = "https://decision-cli.dev/ns#definitionForm";
const DEC_VALUE_STREAM_CLASS: &str = "https://decision-cli.dev/ns#ValueStream";
const DEC_SESSION_CLASS: &str = "https://decision-cli.dev/ns#Session";

/// Distinguishes the two `dec init` input shapes (ADR-006 §3.2).
#[derive(Debug, Clone)]
pub enum DefinitionSource {
    /// `dec init --template <name>` — bundled stream template.
    Template(String),
    /// `dec init --from <path>` — local Turtle file.
    File(PathBuf),
}

impl DefinitionSource {
    fn label(&self) -> String {
        match self {
            Self::Template(n) => format!("template:{n}"),
            Self::File(p) => p.display().to_string(),
        }
    }

    fn form(&self) -> &'static str {
        match self {
            Self::Template(_) => "template",
            Self::File(_) => "file",
        }
    }
}

/// Output of a successful [`run`].
#[derive(Debug, Clone)]
pub struct InitOutcome {
    /// IRI of the persisted ValueStream artifact.
    pub stream_iri: String,
    /// IRI of the persisted ValueAction artifact.
    pub value_action_iri: String,
    /// Bootstrap session IRI (always `dec:session/init-001`).
    pub session_iri: String,
    /// SHA-256 (hex) of the input definition bytes (template *or* file).
    pub definition_hash: String,
    /// Source label as recorded on the session.
    pub definition_source: String,
    /// Ontology version recorded on the session.
    pub ontology_version: String,
    /// Authorized goals copied off the validated stream.
    pub authorized_goals: Vec<String>,
    /// Path to the persisted store directory (`.dec/store/`).
    pub store_dir: PathBuf,
    /// Path to the serialized N-Quads dump inside `store_dir`.
    pub store_dump_path: PathBuf,
}

/// Structured init failures (ADR-006).
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum InitError {
    /// The `.dec/` directory already exists in the working dir.
    #[error("already initialised: {0} exists; refusing to overwrite")]
    AlreadyInitialised(PathBuf),

    /// The named template is not bundled.
    #[error("unknown bundled template '{name}'; available: {available}")]
    UnknownTemplate { name: String, available: String },

    /// Could not read the file passed to `--from`.
    #[error("failed to read definition file {path}: {source}")]
    ReadFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The Turtle did not parse.
    #[error("failed to parse Turtle from {source_label}: {detail}")]
    ParseFailed {
        source_label: String,
        detail: String,
    },

    /// One or more SHACL constraints were violated.
    #[error("SHACL validation failed for {source_label}:\n{report}")]
    ShaclViolation { source_label: String, report: String },

    /// The terminal ValueAction IRI is not in the bundled set.
    #[error(
        "ValueAction <{iri}> is not bundled with slice 1; bundled URIs: {available}. \
         Network resolution is deferred (decision-cli-slice-1-bounds.md §6.2)."
    )]
    UnknownValueAction { iri: String, available: String },

    /// An authorized goal is not in the ValueAction's compatible set.
    #[error(
        "authorized goal {goal:?} is not in the compatible-goals set for {value_action} \
         (compatible: {compatible}). Per ADR-005: this stream pursues {value_action}; \
         try a stream whose value-action accepts {goal:?}."
    )]
    UnauthorizedGoal {
        goal: String,
        value_action: String,
        compatible: String,
    },

    /// The ontology asset itself is malformed (should be impossible).
    #[error("ontology unavailable: {0}")]
    Ontology(#[from] OntologyError),

    /// Underlying I/O failure while persisting `.dec/store/`.
    #[error("failed to persist orchestration store: {0}")]
    PersistFailed(String),

    /// Generic underlying SPARQL / store failure.
    #[error("internal store error: {0}")]
    Internal(String),
}

/// Run the full init pipeline against the supplied source, persisting
/// the orchestration store under `<workdir>/.dec/store/`.
///
/// Errors leave `workdir/.dec/` **absent** — no partial state on
/// failure (TC-003 / TC-004 / TC-005).
pub fn run(workdir: &Path, source: DefinitionSource) -> Result<InitOutcome, InitError> {
    let dec_dir = workdir.join(".dec");
    if dec_dir.exists() {
        return Err(InitError::AlreadyInitialised(dec_dir));
    }

    // -- 0. Load embedded ontology and stage input bytes.
    let ontology = OntologyHandle::load()?;
    let (definition_bytes, source_label, base_iri) = read_definition_bytes(&source)?;
    let definition_hash = sha256_hex(&definition_bytes);

    // -- 1. Parse the stream definition into its own named graph.
    let staging = Store::new().map_err(|e| InitError::Internal(e.to_string()))?;
    let stream_graph = NamedNode::new_unchecked("urn:decision-cli:staging:stream");
    parse_into_graph(
        &staging,
        &definition_bytes,
        &stream_graph,
        &source_label,
        base_iri.as_deref(),
    )?;

    // -- 2. SHACL-validate the stream definition against the embedded shapes.
    let stream_iri = sole_subject_with_class(&staging, &stream_graph, DEC_VALUE_STREAM_CLASS)
        .ok_or_else(|| InitError::ShaclViolation {
            source_label: source_label.clone(),
            report: "definition does not declare any dec:ValueStream instance".to_string(),
        })?;

    let stream_violations = check_stream_shacl(&staging, &stream_graph, &stream_iri, ontology);
    if !stream_violations.is_empty() {
        return Err(InitError::ShaclViolation {
            source_label,
            report: render_violations(&stream_violations),
        });
    }

    // -- 3. Resolve the terminal ValueAction URI against the bundled set.
    let terminal_iri = single_iri_value(&staging, &stream_iri, DEC_TERMINAL_VALUE_ACTION)
        .map_err(|detail| InitError::ShaclViolation {
            source_label: source_label.clone(),
            report: detail,
        })?;
    let bundled_va = bundled::lookup_value_action(&terminal_iri).ok_or_else(|| {
        InitError::UnknownValueAction {
            iri: terminal_iri.clone(),
            available: bundled::VALUE_ACTIONS
                .iter()
                .map(|v| v.iri.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }
    })?;

    // Parse the bundled ValueAction document into the same staging
    // store under its own named graph so we can also SHACL-validate it.
    let va_graph = NamedNode::new_unchecked("urn:decision-cli:staging:value-action");
    parse_into_graph(
        &staging,
        bundled_va.ttl.as_bytes(),
        &va_graph,
        &format!("bundled:{}", bundled_va.iri),
        Some(bundled_va.iri),
    )?;

    let va_violations = check_value_action_shacl(&staging, &va_graph, &terminal_iri);
    if !va_violations.is_empty() {
        // This is a bundled-asset-malformed bug — surface it as a SHACL violation
        // tagged against the bundled source so the failure is loud.
        return Err(InitError::ShaclViolation {
            source_label: format!("bundled:{}", bundled_va.iri),
            report: render_violations(&va_violations),
        });
    }

    // -- 4. Cross-validate authorized goals ⊆ compatible goals.
    let authorized: Vec<String> =
        collect_string_property(&staging, &stream_iri, DEC_AUTHORIZED_GOALS);
    let compatible: Vec<String> =
        collect_string_property(&staging, &terminal_iri, DEC_COMPATIBLE_GOALS);
    let compatible_set: BTreeSet<&str> = compatible.iter().map(String::as_str).collect();

    for goal in &authorized {
        if !compatible_set.contains(goal.as_str()) {
            return Err(InitError::UnauthorizedGoal {
                goal: goal.clone(),
                value_action: terminal_iri.clone(),
                compatible: compatible.join(", "),
            });
        }
    }

    // -- 5. Persist into `<workdir>/.dec/store/` as a single N-Quads dump.
    let store_dir = dec_dir.join("store");
    let dump_path = store_dir.join("orchestration.nq");
    let now = Utc::now().to_rfc3339();

    let orchestration = Store::new().map_err(|e| InitError::Internal(e.to_string()))?;
    // TC-001's audit SPARQL has no FROM/GRAPH clauses, so it targets
    // the default graph; we route the persisted artifacts there.
    // FT-014's `dec:inStream` audit lives in a separate named graph
    // and is exercised by `StreamWriter` (FT-001) — orthogonal.
    let dest_graph = GraphName::DefaultGraph;

    // Copy stream + value-action triples into the default graph.
    copy_triples_default(&staging, &orchestration, &stream_graph, &dest_graph)?;
    copy_triples_default(&staging, &orchestration, &va_graph, &dest_graph)?;

    // Record the bootstrap session (PROV-O) and the wasGeneratedBy
    // links (ADR-004 / TC-015).
    let session_iri = NamedNode::new(BOOTSTRAP_SESSION_IRI)
        .map_err(|e| InitError::Internal(e.to_string()))?;
    let session_quads = build_session_quads(
        &session_iri,
        &dest_graph,
        &source_label,
        &definition_hash,
        ontology.version(),
        source.form(),
        &now,
    );
    orchestration
        .transaction(|mut tx| {
            for q in &session_quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| InitError::Internal(e.to_string()))?;

    let stream_node =
        NamedNode::new(&stream_iri).map_err(|e| InitError::Internal(e.to_string()))?;
    let va_node = NamedNode::new(&terminal_iri).map_err(|e| InitError::Internal(e.to_string()))?;
    let prov_gen = NamedNode::new(PROV_GENERATED_BY)
        .map_err(|e| InitError::Internal(e.to_string()))?;
    let prov_derived = NamedNode::new(PROV_DERIVED_FROM)
        .map_err(|e| InitError::Internal(e.to_string()))?;
    orchestration
        .transaction(|mut tx| {
            tx.insert(
                Quad::new(
                    stream_node.clone(),
                    prov_gen.clone(),
                    session_iri.clone(),
                    dest_graph.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    va_node.clone(),
                    prov_gen.clone(),
                    session_iri.clone(),
                    dest_graph.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    stream_node.clone(),
                    prov_derived.clone(),
                    Literal::new_simple_literal(&source_label),
                    dest_graph.clone(),
                )
                .as_ref(),
            )?;
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| InitError::Internal(e.to_string()))?;

    // -- 6. Serialise the orchestration graph to disk (atomic-ish):
    //       write to a temp path, then rename. Only after the rename
    //       does `.dec/` become observable — preserves the "no state
    //       written on failure" guarantee from ADR-006.
    let tmp_dec = workdir.join(".dec.tmp-init");
    if tmp_dec.exists() {
        fs::remove_dir_all(&tmp_dec).map_err(|e| InitError::PersistFailed(e.to_string()))?;
    }
    fs::create_dir_all(tmp_dec.join("store"))
        .map_err(|e| InitError::PersistFailed(e.to_string()))?;
    let tmp_dump = tmp_dec.join("store").join("orchestration.nq");

    let mut buf = Vec::new();
    orchestration
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .map_err(|e| InitError::PersistFailed(e.to_string()))?;
    fs::write(&tmp_dump, &buf).map_err(|e| InitError::PersistFailed(e.to_string()))?;

    // Drop a `definition.ttl` copy alongside so `dec status` can
    // re-display the source bytes and TC-002 can re-hash from disk.
    fs::write(tmp_dec.join("definition.ttl"), &definition_bytes)
        .map_err(|e| InitError::PersistFailed(e.to_string()))?;
    fs::write(
        tmp_dec.join("init-metadata.json"),
        format!(
            r#"{{"source":"{source}","hash":"{hash}","ontology_version":"{ver}","form":"{form}","stream":"{stream}","value_action":"{va}"}}"#,
            source = json_escape(&source_label),
            hash = definition_hash,
            ver = ontology.version(),
            form = source.form(),
            stream = stream_iri,
            va = terminal_iri,
        ),
    )
    .map_err(|e| InitError::PersistFailed(e.to_string()))?;

    fs::rename(&tmp_dec, &dec_dir).map_err(|e| InitError::PersistFailed(e.to_string()))?;

    Ok(InitOutcome {
        stream_iri,
        value_action_iri: terminal_iri,
        session_iri: BOOTSTRAP_SESSION_IRI.to_string(),
        definition_hash,
        definition_source: source_label,
        ontology_version: ontology.version().to_string(),
        authorized_goals: authorized,
        store_dir,
        store_dump_path: dump_path,
    })
}

fn read_definition_bytes(
    source: &DefinitionSource,
) -> Result<(Vec<u8>, String, Option<String>), InitError> {
    match source {
        DefinitionSource::Template(name) => {
            let t = bundled::lookup_stream_template(name).ok_or_else(|| {
                InitError::UnknownTemplate {
                    name: name.clone(),
                    available: bundled::known_template_names().join(", "),
                }
            })?;
            Ok((t.ttl.as_bytes().to_vec(), source.label(), Some(t.iri.to_string())))
        }
        DefinitionSource::File(path) => {
            let bytes = fs::read(path).map_err(|source| InitError::ReadFailed {
                path: path.clone(),
                source,
            })?;
            Ok((bytes, source.label(), None))
        }
    }
}

fn parse_into_graph(
    store: &Store,
    bytes: &[u8],
    graph: &NamedNode,
    source_label: &str,
    base_iri: Option<&str>,
) -> Result<(), InitError> {
    let mut parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(graph.clone()));
    if let Some(b) = base_iri {
        parser = parser
            .with_base_iri(b)
            .map_err(|e| InitError::ParseFailed {
                source_label: source_label.to_string(),
                detail: format!("invalid base IRI {b}: {e}"),
            })?;
    }
    store
        .load_from_reader(parser, bytes)
        .map_err(|e| InitError::ParseFailed {
            source_label: source_label.to_string(),
            detail: e.to_string(),
        })?;
    Ok(())
}

fn sole_subject_with_class(store: &Store, graph: &NamedNode, class: &str) -> Option<String> {
    let q = format!(
        "SELECT ?s WHERE {{ GRAPH <{g}> {{ ?s a <{cls}> }} }} LIMIT 2",
        g = graph.as_str(),
        cls = class,
    );
    let res = store.query(q.as_str()).ok()?;
    let QueryResults::Solutions(mut sols) = res else {
        return None;
    };
    let first = sols.next()?.ok()?;
    if sols.next().is_some() {
        // More than one — the definition is ambiguous; signal as None
        // and let the caller surface a SHACL-style violation.
        return None;
    }
    if let Some(Term::NamedNode(n)) = first.get("s") {
        Some(n.as_str().to_string())
    } else {
        None
    }
}

#[derive(Debug)]
struct Violation {
    target: String,
    path: String,
    detail: String,
}

fn check_stream_shacl(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
    _ontology: &OntologyHandle,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let required = [
        (DEC_NAME, true /* string */, true /* single */),
        (DEC_TITLE, true, true),
    ];
    for (prop, is_string, single) in required {
        let values = collect_property_values(store, graph, subject_iri, prop);
        if values.is_empty() {
            violations.push(Violation {
                target: subject_iri.to_string(),
                path: prop.to_string(),
                detail: format!("missing required property {prop} (sh:minCount 1)"),
            });
            continue;
        }
        if single && values.len() > 1 {
            violations.push(Violation {
                target: subject_iri.to_string(),
                path: prop.to_string(),
                detail: format!(
                    "expected exactly one {prop}, found {} (sh:maxCount 1)",
                    values.len()
                ),
            });
        }
        if is_string {
            for v in &values {
                if !matches!(v, Term::Literal(_)) {
                    violations.push(Violation {
                        target: subject_iri.to_string(),
                        path: prop.to_string(),
                        detail: format!(
                            "{prop} must be a literal (sh:datatype xsd:string), got {}",
                            term_kind(v)
                        ),
                    });
                }
            }
        }
    }

    // terminalValueAction: exactly one IRI.
    let tvalues = collect_property_values(store, graph, subject_iri, DEC_TERMINAL_VALUE_ACTION);
    if tvalues.is_empty() {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_TERMINAL_VALUE_ACTION.to_string(),
            detail: format!(
                "missing required property {DEC_TERMINAL_VALUE_ACTION} (sh:minCount 1)"
            ),
        });
    } else if tvalues.len() > 1 {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_TERMINAL_VALUE_ACTION.to_string(),
            detail: format!(
                "expected exactly one {DEC_TERMINAL_VALUE_ACTION}, found {}",
                tvalues.len()
            ),
        });
    } else if !matches!(&tvalues[0], Term::NamedNode(_)) {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_TERMINAL_VALUE_ACTION.to_string(),
            detail: format!(
                "{DEC_TERMINAL_VALUE_ACTION} must be an IRI (sh:nodeKind sh:IRI)"
            ),
        });
    }

    // authorizedGoals: at least one (literal or list).
    let authorized = collect_string_property(store, subject_iri, DEC_AUTHORIZED_GOALS);
    if authorized.is_empty() {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_AUTHORIZED_GOALS.to_string(),
            detail: format!(
                "missing required property {DEC_AUTHORIZED_GOALS} (sh:minCount 1)"
            ),
        });
    }
    violations
}

fn check_value_action_shacl(store: &Store, graph: &NamedNode, subject_iri: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (prop, must_have_string, single) in [
        (DEC_NAME, true, true),
        (DEC_DESCRIPTION, true, false),
        (DEC_COMPATIBLE_GOALS, false, false),
        ("https://decision-cli.dev/ns#exitCriterion", false, false),
        ("https://decision-cli.dev/ns#expectedOutputType", false, false),
    ] {
        let values = collect_property_values(store, graph, subject_iri, prop);
        if values.is_empty() {
            violations.push(Violation {
                target: subject_iri.to_string(),
                path: prop.to_string(),
                detail: format!("missing required property {prop} (sh:minCount 1)"),
            });
            continue;
        }
        if single && values.len() > 1 {
            violations.push(Violation {
                target: subject_iri.to_string(),
                path: prop.to_string(),
                detail: format!(
                    "expected exactly one {prop}, found {} (sh:maxCount 1)",
                    values.len()
                ),
            });
        }
        if must_have_string {
            for v in &values {
                if !matches!(v, Term::Literal(_)) {
                    violations.push(Violation {
                        target: subject_iri.to_string(),
                        path: prop.to_string(),
                        detail: format!("{prop} must be a literal, got {}", term_kind(v)),
                    });
                }
            }
        }
    }
    violations
}

fn render_violations(vs: &[Violation]) -> String {
    let mut out = String::new();
    for v in vs {
        out.push_str(&format!(
            "  • subject <{}> path <{}>: {}\n",
            v.target, v.path, v.detail
        ));
    }
    out
}

fn collect_property_values(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
    prop: &str,
) -> Vec<Term> {
    let q = format!(
        "SELECT ?o WHERE {{ GRAPH <{g}> {{ <{s}> <{p}> ?o }} }}",
        g = graph.as_str(),
        s = subject_iri,
        p = prop,
    );
    let mut out = Vec::new();
    let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) else {
        return out;
    };
    for sol in sols {
        let Ok(sol) = sol else { continue };
        if let Some(t) = sol.get("o") {
            out.push(t.clone());
        }
    }
    out
}

fn single_iri_value(store: &Store, subject_iri: &str, prop: &str) -> Result<String, String> {
    let q = format!(
        "SELECT ?o WHERE {{ GRAPH ?g {{ <{s}> <{p}> ?o }} }}",
        s = subject_iri,
        p = prop,
    );
    let res = store
        .query(q.as_str())
        .map_err(|e| format!("internal SPARQL error: {e}"))?;
    let QueryResults::Solutions(sols) = res else {
        return Err(format!("SPARQL for {prop} returned non-solutions"));
    };
    let mut iris = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| format!("internal SPARQL error: {e}"))?;
        if let Some(Term::NamedNode(n)) = sol.get("o") {
            iris.push(n.as_str().to_string());
        }
    }
    match iris.len() {
        0 => Err(format!("no IRI value for {prop} on {subject_iri}")),
        1 => Ok(iris.remove(0)),
        n => Err(format!("expected exactly one IRI for {prop}, found {n}")),
    }
}

/// Collect plain-string property values, accepting either repeated
/// triples or an RDF collection (Turtle list).
fn collect_string_property(store: &Store, subject_iri: &str, prop: &str) -> Vec<String> {
    let mut out = Vec::new();

    // Repeated-triple form: <s> <p> "v".
    let q = format!(
        "SELECT ?o WHERE {{ GRAPH ?g {{ <{s}> <{p}> ?o }} }}",
        s = subject_iri,
        p = prop,
    );
    if let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) {
        for sol in sols.flatten() {
            if let Some(t) = sol.get("o") {
                match t {
                    Term::Literal(lit) => out.push(lit.value().to_string()),
                    Term::BlankNode(b) => {
                        // RDF collection — walk the list.
                        let head_iri = format!("_:{}", b.as_str());
                        walk_list(store, &head_iri, &mut out);
                    }
                    Term::NamedNode(n) => {
                        if n.as_str() == RDF_NIL {
                            // empty list
                        } else {
                            walk_list(store, n.as_str(), &mut out);
                        }
                    }
                    Term::Triple(_) => {}
                }
            }
        }
    }
    out
}

fn walk_list(store: &Store, head: &str, out: &mut Vec<String>) {
    // We accept both blank-node and named-node list heads.
    // SPARQL "head rdf:first ?v" + "head rdf:rest ?next" loop.
    let mut current = head.to_string();
    loop {
        if current == RDF_NIL {
            return;
        }
        let first_q = if current.starts_with("_:") {
            let id = &current[2..];
            format!(
                "SELECT ?v WHERE {{ GRAPH ?g {{ _:{id} <{first}> ?v }} }}",
                first = RDF_FIRST,
            )
        } else {
            format!(
                "SELECT ?v WHERE {{ GRAPH ?g {{ <{current}> <{first}> ?v }} }}",
                first = RDF_FIRST,
            )
        };
        let mut found_value = false;
        if let Ok(QueryResults::Solutions(sols)) = store.query(first_q.as_str()) {
            for sol in sols.flatten() {
                if let Some(Term::Literal(lit)) = sol.get("v") {
                    out.push(lit.value().to_string());
                    found_value = true;
                }
            }
        }
        if !found_value {
            return;
        }
        let rest_q = if current.starts_with("_:") {
            let id = &current[2..];
            format!(
                "SELECT ?v WHERE {{ GRAPH ?g {{ _:{id} <{rest}> ?v }} }}",
                rest = RDF_REST,
            )
        } else {
            format!(
                "SELECT ?v WHERE {{ GRAPH ?g {{ <{current}> <{rest}> ?v }} }}",
                rest = RDF_REST,
            )
        };
        let mut next = String::new();
        if let Ok(QueryResults::Solutions(sols)) = store.query(rest_q.as_str()) {
            for sol in sols.flatten() {
                if let Some(term) = sol.get("v") {
                    match term {
                        Term::NamedNode(n) => next = n.as_str().to_string(),
                        Term::BlankNode(b) => next = format!("_:{}", b.as_str()),
                        _ => {}
                    }
                }
            }
        }
        if next.is_empty() || next == current {
            return;
        }
        current = next;
    }
}

fn term_kind(t: &Term) -> &'static str {
    match t {
        Term::NamedNode(_) => "IRI",
        Term::BlankNode(_) => "blank node",
        Term::Literal(_) => "literal",
        Term::Triple(_) => "embedded triple",
    }
}

fn copy_triples_default(
    src: &Store,
    dest: &Store,
    src_graph: &NamedNode,
    dest_graph: &GraphName,
) -> Result<(), InitError> {
    let g_ref = GraphNameRef::NamedNode(src_graph.as_ref());
    let mut to_insert: Vec<Quad> = Vec::new();
    for q in src.quads_for_pattern(None, None, None, Some(g_ref)) {
        let q = q.map_err(|e| InitError::Internal(e.to_string()))?;
        let nq = Quad::new(
            q.subject.clone(),
            q.predicate.clone(),
            q.object.clone(),
            dest_graph.clone(),
        );
        to_insert.push(nq);
    }
    dest.transaction(|mut tx| {
        for q in &to_insert {
            tx.insert(q.as_ref())?;
        }
        Ok::<_, oxigraph::store::StorageError>(())
    })
    .map_err(|e| InitError::Internal(e.to_string()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_session_quads(
    session_iri: &NamedNode,
    graph: &GraphName,
    source_label: &str,
    definition_hash: &str,
    ontology_version: &str,
    form: &str,
    started_at: &str,
) -> Vec<Quad> {
    let g: GraphName = graph.clone();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let session_class = NamedNodeRef::new_unchecked(DEC_SESSION_CLASS);
    let activity = NamedNodeRef::new_unchecked(PROV_ACTIVITY);
    let derived = NamedNodeRef::new_unchecked(PROV_DERIVED_FROM);
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME);
    let p_source = NamedNodeRef::new_unchecked(DEC_DEFINITION_SOURCE);
    let p_hash = NamedNodeRef::new_unchecked(DEC_DEFINITION_HASH);
    let p_ver = NamedNodeRef::new_unchecked(DEC_ONTOLOGY_VERSION);
    let p_form = NamedNodeRef::new_unchecked(DEC_DEFINITION_FORM);
    vec![
        Quad::new(session_iri.clone(), rdf_type, session_class, g.clone()),
        Quad::new(session_iri.clone(), rdf_type, activity, g.clone()),
        Quad::new(
            session_iri.clone(),
            p_source,
            Literal::new_simple_literal(source_label),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            derived,
            Literal::new_simple_literal(source_label),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            p_hash,
            Literal::new_simple_literal(definition_hash),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            p_ver,
            Literal::new_simple_literal(ontology_version),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            p_form,
            Literal::new_simple_literal(form),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            at_time,
            Literal::new_simple_literal(started_at),
            g.clone(),
        ),
    ]
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// `Subject` import is kept for future RDF-list edge cases.
#[allow(dead_code)]
fn _retain_subject_import() -> Option<Subject> {
    None
}
