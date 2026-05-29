//! Reading `dec:VerificationGraph` Turtle files into in-memory models
//! (FT-036 / ADR-028). The canonical serialiser lives in the sibling
//! [`super::serialize`] module.
//!
//! On-disk Turtle is authoritative; the orchestration store projection
//! is rebuilt from disk on every load.

use std::fs;
use std::path::{Path, PathBuf};

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{GraphName, NamedNode, Term};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::vocab::{
    IRI_DEC_BENCH, IRI_DEC_PROVIDES_EVIDENCE_FOR, IRI_DEC_STEPS, IRI_DEC_STEP_TYPE,
    IRI_DEC_VERIFICATION_GRAPH, IRI_DEC_VERIFIES, IRI_DEC_VERIFY_GRAPH_PREFIX,
};

use super::parse::{parse_kind_fields, read_iri_list, single_iri, single_literal};
pub use super::serialize::to_canonical_turtle;
use super::shacl::UnknownStepKindError;
use super::types::{
    ArtifactRef, StepKind, VerificationGraph, VerificationStep, RDF_FIRST, RDF_NIL, RDF_REST,
};

/// Failures produced by [`from_turtle`].
#[derive(Debug, Error)]
pub enum GraphIoError {
    /// `fs::read` failure.
    #[error("failed to read graph file {path}: {source}")]
    ReadFailed {
        /// Path that failed to load.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Turtle parser rejected the input.
    #[error("failed to parse Turtle from {path}: {detail}")]
    ParseFailed {
        /// Path whose contents failed to parse.
        path: PathBuf,
        /// Parser-supplied diagnostic.
        detail: String,
    },
    /// File parses, but its content does not form a valid graph.
    #[error("graph file {path} has malformed shape: {detail}")]
    MalformedShape {
        /// Path whose graph shape is invalid.
        path: PathBuf,
        /// Why the shape is invalid.
        detail: String,
    },
    /// `dec:stepType` literal is not in the seed vocabulary.
    #[error("graph file {path}: {source}")]
    UnknownStepKind {
        /// Path whose graph contained the unknown step kind.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: UnknownStepKindError,
    },
}

/// Parse a single graph from a `.ttl` file on disk.
pub fn from_turtle(path: &Path) -> Result<VerificationGraph, GraphIoError> {
    let bytes = fs::read(path).map_err(|source| GraphIoError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })?;
    from_turtle_bytes(path, &bytes)
}

/// Variant of [`from_turtle`] that consumes bytes directly. The `path`
/// argument is only used for error reporting.
pub fn from_turtle_bytes(path: &Path, bytes: &[u8]) -> Result<VerificationGraph, GraphIoError> {
    let store = new_staging_store(path)?;
    let graph_iri = "urn:decision-cli:verify-graph-parse-staging";
    let graph = NamedNode::new_unchecked(graph_iri);
    let parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(graph));
    store
        .load_from_reader(parser, bytes)
        .map_err(|e| GraphIoError::ParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    let subject = find_graph_subject(&store, path)?;
    extract_graph(&store, &subject, path)
}

fn new_staging_store(path: &Path) -> Result<Store, GraphIoError> {
    Store::new().map_err(|e| GraphIoError::ParseFailed {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })
}

fn find_graph_subject(store: &Store, path: &Path) -> Result<NamedNode, GraphIoError> {
    let q = format!(
        "SELECT ?s WHERE {{ GRAPH ?g {{ ?s a <{cls}> }} }}",
        cls = IRI_DEC_VERIFICATION_GRAPH,
    );
    let res = store
        .query(q.as_str())
        .map_err(|e| GraphIoError::ParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    let oxigraph::sparql::QueryResults::Solutions(sols) = res else {
        return Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "SPARQL query for VerificationGraph returned non-solutions".to_string(),
        });
    };
    let subjects = collect_graph_subjects(sols, path)?;
    pick_single_graph_subject(subjects, path)
}

fn collect_graph_subjects(
    sols: oxigraph::sparql::QuerySolutionIter,
    path: &Path,
) -> Result<Vec<NamedNode>, GraphIoError> {
    let mut subjects: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| GraphIoError::ParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
        if let Some(Term::NamedNode(n)) = sol.get("s") {
            subjects.push(n.clone());
        }
    }
    Ok(subjects)
}

fn pick_single_graph_subject(
    mut subjects: Vec<NamedNode>,
    path: &Path,
) -> Result<NamedNode, GraphIoError> {
    match subjects.len() {
        0 => Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "no dec:VerificationGraph subject in file".to_string(),
        }),
        1 => Ok(subjects.remove(0)),
        n => Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("expected exactly one VerificationGraph subject, found {n}"),
        }),
    }
}

fn extract_graph(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<VerificationGraph, GraphIoError> {
    let verifies = single_iri(store, subject, IRI_DEC_VERIFIES, path)?.ok_or_else(|| {
        GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "missing dec:verifies".to_string(),
        }
    })?;
    let environment = single_iri(store, subject, IRI_DEC_BENCH, path)?.ok_or_else(|| {
        GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "missing dec:bench".to_string(),
        }
    })?;
    let steps = read_steps_list(store, subject, path)?;
    let id_str = subject
        .as_str()
        .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
        .unwrap_or(subject.as_str())
        .to_string();
    let step_models = parse_steps(store, &id_str, &steps, path)?;
    Ok(VerificationGraph {
        id: subject.clone(),
        verifies: ArtifactRef(verifies),
        environment,
        steps: step_models,
    })
}

fn read_steps_list(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<Vec<NamedNode>, GraphIoError> {
    let mut heads: Vec<Term> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(subject.clone()).as_ref()),
            Some(NamedNode::new_unchecked(IRI_DEC_STEPS).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        heads.push(quad.object);
    }
    if heads.is_empty() {
        return Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "missing dec:steps rdf:List".to_string(),
        });
    }
    if heads.len() > 1 {
        return Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!(
                "expected exactly one dec:steps list head, found {}",
                heads.len()
            ),
        });
    }
    let head = heads.remove(0);
    walk_steps_list(store, head, path)
}

fn walk_steps_list(store: &Store, head: Term, path: &Path) -> Result<Vec<NamedNode>, GraphIoError> {
    let mut out: Vec<NamedNode> = Vec::new();
    let mut current = head;
    let mut seen = std::collections::BTreeSet::<String>::new();
    loop {
        if matches!(&current, Term::NamedNode(n) if n.as_str() == RDF_NIL) {
            break;
        }
        let key = match &current {
            Term::BlankNode(b) => format!("bn:{}", b.as_str()),
            Term::NamedNode(n) => format!("iri:{}", n.as_str()),
            _ => {
                return Err(GraphIoError::MalformedShape {
                    path: path.to_path_buf(),
                    detail: "dec:steps list node has unsupported term shape".to_string(),
                })
            }
        };
        if !seen.insert(key) {
            return Err(GraphIoError::MalformedShape {
                path: path.to_path_buf(),
                detail: "dec:steps list is cyclic".to_string(),
            });
        }
        let first_value = list_first_iri(store, &current, path)?;
        out.push(first_value);
        current = list_rest(store, &current, path)?;
    }
    Ok(out)
}

fn list_first_iri(store: &Store, head: &Term, path: &Path) -> Result<NamedNode, GraphIoError> {
    for quad in store
        .quads_for_pattern(
            Some(term_to_subject_ref(head, path)?.as_ref()),
            Some(NamedNode::new_unchecked(RDF_FIRST).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::NamedNode(n) = quad.object {
            return Ok(n);
        }
    }
    Err(GraphIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: "dec:steps list node missing rdf:first IRI".to_string(),
    })
}

fn list_rest(store: &Store, head: &Term, path: &Path) -> Result<Term, GraphIoError> {
    for quad in store
        .quads_for_pattern(
            Some(term_to_subject_ref(head, path)?.as_ref()),
            Some(NamedNode::new_unchecked(RDF_REST).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        return Ok(quad.object);
    }
    Err(GraphIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: "dec:steps list node missing rdf:rest".to_string(),
    })
}

fn term_to_subject_ref(t: &Term, path: &Path) -> Result<oxigraph::model::Subject, GraphIoError> {
    match t {
        Term::NamedNode(n) => Ok(oxigraph::model::Subject::NamedNode(n.clone())),
        Term::BlankNode(b) => Ok(oxigraph::model::Subject::BlankNode(b.clone())),
        _ => Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "rdf:List node must be IRI or blank node".to_string(),
        }),
    }
}

fn parse_steps(
    store: &Store,
    _graph_id: &str,
    step_iris: &[NamedNode],
    path: &Path,
) -> Result<Vec<VerificationStep>, GraphIoError> {
    let mut out: Vec<VerificationStep> = Vec::with_capacity(step_iris.len());
    for iri in step_iris.iter() {
        let kind_raw = single_literal(store, iri, IRI_DEC_STEP_TYPE, path)?.ok_or_else(|| {
            GraphIoError::MalformedShape {
                path: path.to_path_buf(),
                detail: format!("step <{}> missing dec:stepType", iri.as_str()),
            }
        })?;
        let kind = StepKind::parse(&kind_raw).ok_or_else(|| GraphIoError::UnknownStepKind {
            path: path.to_path_buf(),
            source: UnknownStepKindError {
                value: kind_raw.clone(),
            },
        })?;
        let fields = parse_kind_fields(store, iri, kind, path)?;
        let provides_evidence_for = read_iri_list(store, iri, IRI_DEC_PROVIDES_EVIDENCE_FOR);
        out.push(VerificationStep {
            id: iri.clone(),
            kind,
            fields,
            provides_evidence_for,
        });
    }
    Ok(out)
}
