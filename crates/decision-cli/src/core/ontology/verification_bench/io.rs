//! On-disk Turtle (de)serialisation for `dec:VerificationBench`.
//!
//! On-disk Turtle is authoritative per ADR-028 §State. The canonical
//! writer here produces byte-identical output for the same input, which
//! is what TC-055 relies on for the seed-reproducibility check.

use std::fs;
use std::path::{Path, PathBuf};

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{GraphName, NamedNode, Term};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::vocab::{
    IRI_DEC_ALLOWED_OPS, IRI_DEC_ENDPOINT, IRI_DEC_BENCH_PREFIX, IRI_DEC_BENCH_TYPE,
    IRI_DEC_FIXTURE_SOURCE, IRI_DEC_SAFETY_CLASS, IRI_DEC_SETUP, IRI_DEC_TEARDOWN,
    IRI_DEC_VERIFICATION_BENCH,
};

use super::types::{SafetyClass, VerificationBench, RDF_FIRST, RDF_NIL, RDF_REST};

/// Failures produced by [`from_turtle`].
#[derive(Debug, Error)]
pub enum EnvIoError {
    /// `fs::read` failure (missing file, permission denied, etc.).
    #[error("failed to read bench file {path}: {source}")]
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
    /// File parses, but its content does not form a valid bench.
    #[error("bench file {path} has malformed shape: {detail}")]
    MalformedShape {
        /// Path whose bench shape is invalid.
        path: PathBuf,
        /// Why the shape is invalid.
        detail: String,
    },
}

/// Parse a single environment from a `.ttl` file on disk.
pub fn from_turtle(path: &Path) -> Result<VerificationBench, EnvIoError> {
    let bytes = fs::read(path).map_err(|source| EnvIoError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })?;
    from_turtle_bytes(path, &bytes)
}

/// Variant of [`from_turtle`] that consumes bytes directly. The `path`
/// argument is only used for error reporting.
pub fn from_turtle_bytes(path: &Path, bytes: &[u8]) -> Result<VerificationBench, EnvIoError> {
    let store = Store::new().map_err(|e| EnvIoError::ParseFailed {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let graph_iri = "urn:decision-cli:bench-parse-staging";
    let graph = NamedNode::new_unchecked(graph_iri);
    let parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(graph));
    store
        .load_from_reader(parser, bytes)
        .map_err(|e| EnvIoError::ParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    let subject = find_env_subject(&store, path)?;
    extract_env(&store, &subject, path)
}

fn find_env_subject(store: &Store, path: &Path) -> Result<NamedNode, EnvIoError> {
    let q = format!(
        "SELECT ?s WHERE {{ GRAPH ?g {{ ?s a <{cls}> }} }}",
        cls = IRI_DEC_VERIFICATION_BENCH,
    );
    let res = store
        .query(q.as_str())
        .map_err(|e| EnvIoError::ParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    let oxigraph::sparql::QueryResults::Solutions(sols) = res else {
        return Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "SPARQL query for VerificationBench returned non-solutions".to_string(),
        });
    };
    let subjects = collect_env_subjects(sols, path)?;
    pick_single_env_subject(subjects, path)
}

fn collect_env_subjects(
    sols: oxigraph::sparql::QuerySolutionIter,
    path: &Path,
) -> Result<Vec<NamedNode>, EnvIoError> {
    let mut subjects: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| EnvIoError::ParseFailed {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
        if let Some(Term::NamedNode(n)) = sol.get("s") {
            subjects.push(n.clone());
        }
    }
    Ok(subjects)
}

fn pick_single_env_subject(
    mut subjects: Vec<NamedNode>,
    path: &Path,
) -> Result<NamedNode, EnvIoError> {
    match subjects.len() {
        0 => Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "no dec:VerificationBench subject in file".to_string(),
        }),
        1 => Ok(subjects.remove(0)),
        n => Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("expected exactly one VerificationBench subject, found {n}"),
        }),
    }
}

fn extract_env(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<VerificationBench, EnvIoError> {
    let id = extract_id(subject, path)?;
    let bench_type = require_literal(
        store,
        subject,
        IRI_DEC_BENCH_TYPE,
        "missing dec:benchType",
        path,
    )?;
    let safety_class = extract_safety_class(store, subject, path)?;
    let setup = single_literal(store, subject, IRI_DEC_SETUP, path)?;
    let teardown = single_literal(store, subject, IRI_DEC_TEARDOWN, path)?;
    let endpoint = single_literal(store, subject, IRI_DEC_ENDPOINT, path)?;
    let fixture_source = single_literal(store, subject, IRI_DEC_FIXTURE_SOURCE, path)?;
    let allowed_ops = read_allowed_ops_list(store, subject, path)?;
    Ok(VerificationBench {
        id,
        bench_type,
        setup,
        teardown,
        allowed_ops,
        safety_class,
        endpoint,
        fixture_source,
    })
}

fn extract_id(subject: &NamedNode, path: &Path) -> Result<String, EnvIoError> {
    id_from_iri(subject.as_str()).ok_or_else(|| EnvIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: format!(
            "subject IRI {iri:?} does not start with {prefix}",
            iri = subject.as_str(),
            prefix = IRI_DEC_BENCH_PREFIX
        ),
    })
}

fn require_literal(
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
    missing_detail: &str,
    path: &Path,
) -> Result<String, EnvIoError> {
    single_literal(store, subject, predicate, path)?.ok_or_else(|| EnvIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: missing_detail.to_string(),
    })
}

fn extract_safety_class(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<SafetyClass, EnvIoError> {
    let safety_raw = require_literal(
        store,
        subject,
        IRI_DEC_SAFETY_CLASS,
        "missing dec:safetyClass",
        path,
    )?;
    SafetyClass::parse(&safety_raw).ok_or_else(|| EnvIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: format!("unknown dec:safetyClass value {safety_raw:?}"),
    })
}

fn id_from_iri(iri: &str) -> Option<String> {
    iri.strip_prefix(IRI_DEC_BENCH_PREFIX).map(|s| s.to_string())
}

fn single_literal(
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
    path: &Path,
) -> Result<Option<String>, EnvIoError> {
    let mut out: Vec<String> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(subject.clone()).as_ref()),
            Some(NamedNode::new_unchecked(predicate).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::Literal(lit) = quad.object {
            out.push(lit.value().to_string());
        }
    }
    match out.len() {
        0 => Ok(None),
        1 => Ok(Some(out.remove(0))),
        n => Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

fn read_allowed_ops_list(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<Vec<String>, EnvIoError> {
    let head = pick_allowed_ops_head(store, subject, path)?;
    walk_allowed_ops_list(store, head, path)
}

fn pick_allowed_ops_head(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<Term, EnvIoError> {
    let mut heads: Vec<Term> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(subject.clone()).as_ref()),
            Some(NamedNode::new_unchecked(IRI_DEC_ALLOWED_OPS).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        heads.push(quad.object);
    }
    if heads.is_empty() {
        return Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "missing dec:allowedOps rdf:List".to_string(),
        });
    }
    if heads.len() > 1 {
        return Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!(
                "expected exactly one dec:allowedOps list, found {}",
                heads.len()
            ),
        });
    }
    Ok(heads.remove(0))
}

fn walk_allowed_ops_list(
    store: &Store,
    head: Term,
    path: &Path,
) -> Result<Vec<String>, EnvIoError> {
    let mut out: Vec<String> = Vec::new();
    let mut current = head;
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    loop {
        if matches!(&current, Term::NamedNode(n) if n.as_str() == RDF_NIL) {
            break;
        }
        let key = allowed_ops_term_key(&current, path)?;
        if !seen.insert(key) {
            return Err(EnvIoError::MalformedShape {
                path: path.to_path_buf(),
                detail: "dec:allowedOps list is cyclic".to_string(),
            });
        }
        out.push(first_value_for(store, &current)?);
        current = rest_for(store, &current)?;
    }
    Ok(out)
}

fn allowed_ops_term_key(current: &Term, path: &Path) -> Result<String, EnvIoError> {
    match current {
        Term::BlankNode(b) => Ok(format!("bn:{}", b.as_str())),
        Term::NamedNode(n) => Ok(format!("iri:{}", n.as_str())),
        _ => Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "dec:allowedOps list node has unsupported term shape".to_string(),
        }),
    }
}

fn first_value_for(store: &Store, head: &Term) -> Result<String, EnvIoError> {
    for quad in store
        .quads_for_pattern(
            Some(term_to_subject_ref(head)?.as_ref()),
            Some(NamedNode::new_unchecked(RDF_FIRST).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::Literal(lit) = quad.object {
            return Ok(lit.value().to_string());
        }
    }
    Err(EnvIoError::MalformedShape {
        path: PathBuf::new(),
        detail: "dec:allowedOps list node missing rdf:first literal".to_string(),
    })
}

fn rest_for(store: &Store, head: &Term) -> Result<Term, EnvIoError> {
    for quad in store
        .quads_for_pattern(
            Some(term_to_subject_ref(head)?.as_ref()),
            Some(NamedNode::new_unchecked(RDF_REST).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        return Ok(quad.object);
    }
    Err(EnvIoError::MalformedShape {
        path: PathBuf::new(),
        detail: "dec:allowedOps list node missing rdf:rest".to_string(),
    })
}

fn term_to_subject_ref(t: &Term) -> Result<oxigraph::model::Subject, EnvIoError> {
    match t {
        Term::NamedNode(n) => Ok(oxigraph::model::Subject::NamedNode(n.clone())),
        Term::BlankNode(b) => Ok(oxigraph::model::Subject::BlankNode(b.clone())),
        _ => Err(EnvIoError::MalformedShape {
            path: PathBuf::new(),
            detail: "rdf:List node must be IRI or blank node".to_string(),
        }),
    }
}
