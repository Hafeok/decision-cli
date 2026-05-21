//! On-disk Turtle (de)serialisation for `dec:VerificationEnvironment`.
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
    IRI_DEC_ALLOWED_OPS, IRI_DEC_ENDPOINT, IRI_DEC_ENV_PREFIX, IRI_DEC_ENV_TYPE,
    IRI_DEC_SAFETY_CLASS, IRI_DEC_SETUP, IRI_DEC_TEARDOWN, IRI_DEC_VERIFICATION_ENVIRONMENT,
};

use super::types::{SafetyClass, VerificationEnvironment, RDF_FIRST, RDF_NIL, RDF_REST};

/// Failures produced by [`from_turtle`].
#[derive(Debug, Error)]
pub enum EnvIoError {
    /// `fs::read` failure (missing file, permission denied, etc.).
    #[error("failed to read env file {path}: {source}")]
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
    /// File parses, but its content does not form a valid env.
    #[error("env file {path} has malformed shape: {detail}")]
    MalformedShape {
        /// Path whose env shape is invalid.
        path: PathBuf,
        /// Why the shape is invalid.
        detail: String,
    },
}

/// Parse a single environment from a `.ttl` file on disk.
pub fn from_turtle(path: &Path) -> Result<VerificationEnvironment, EnvIoError> {
    let bytes = fs::read(path).map_err(|source| EnvIoError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })?;
    from_turtle_bytes(path, &bytes)
}

/// Variant of [`from_turtle`] that consumes bytes directly. The `path`
/// argument is only used for error reporting.
pub fn from_turtle_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<VerificationEnvironment, EnvIoError> {
    let store = Store::new().map_err(|e| EnvIoError::ParseFailed {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let graph_iri = "urn:decision-cli:env-parse-staging";
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
        cls = IRI_DEC_VERIFICATION_ENVIRONMENT,
    );
    let res = store.query(q.as_str()).map_err(|e| EnvIoError::ParseFailed {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let oxigraph::sparql::QueryResults::Solutions(sols) = res else {
        return Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "SPARQL query for VerificationEnvironment returned non-solutions".to_string(),
        });
    };
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
    match subjects.len() {
        0 => Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "no dec:VerificationEnvironment subject in file".to_string(),
        }),
        1 => Ok(subjects.remove(0)),
        n => Err(EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("expected exactly one VerificationEnvironment subject, found {n}"),
        }),
    }
}

fn extract_env(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<VerificationEnvironment, EnvIoError> {
    let id = id_from_iri(subject.as_str()).ok_or_else(|| EnvIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: format!(
            "subject IRI {iri:?} does not start with {prefix}",
            iri = subject.as_str(),
            prefix = IRI_DEC_ENV_PREFIX
        ),
    })?;
    let env_type = single_literal(store, subject, IRI_DEC_ENV_TYPE, path)?.ok_or_else(|| {
        EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "missing dec:envType".to_string(),
        }
    })?;
    let safety_raw = single_literal(store, subject, IRI_DEC_SAFETY_CLASS, path)?.ok_or_else(
        || EnvIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: "missing dec:safetyClass".to_string(),
        },
    )?;
    let safety_class = SafetyClass::parse(&safety_raw).ok_or_else(|| EnvIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: format!("unknown dec:safetyClass value {safety_raw:?}"),
    })?;
    let setup = single_literal(store, subject, IRI_DEC_SETUP, path)?;
    let teardown = single_literal(store, subject, IRI_DEC_TEARDOWN, path)?;
    let endpoint = single_literal(store, subject, IRI_DEC_ENDPOINT, path)?;
    let allowed_ops = read_allowed_ops_list(store, subject, path)?;
    Ok(VerificationEnvironment {
        id,
        env_type,
        setup,
        teardown,
        allowed_ops,
        safety_class,
        endpoint,
    })
}

fn id_from_iri(iri: &str) -> Option<String> {
    iri.strip_prefix(IRI_DEC_ENV_PREFIX).map(|s| s.to_string())
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
            detail: format!("expected exactly one dec:allowedOps list, found {}", heads.len()),
        });
    }
    let head = heads.remove(0);
    let mut out: Vec<String> = Vec::new();
    let mut current = head;
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    loop {
        if matches!(&current, Term::NamedNode(n) if n.as_str() == RDF_NIL) {
            break;
        }
        let key = match &current {
            Term::BlankNode(b) => format!("bn:{}", b.as_str()),
            Term::NamedNode(n) => format!("iri:{}", n.as_str()),
            _ => {
                return Err(EnvIoError::MalformedShape {
                    path: path.to_path_buf(),
                    detail: "dec:allowedOps list node has unsupported term shape".to_string(),
                })
            }
        };
        if !seen.insert(key) {
            return Err(EnvIoError::MalformedShape {
                path: path.to_path_buf(),
                detail: "dec:allowedOps list is cyclic".to_string(),
            });
        }
        let first_value = first_value_for(store, &current)?;
        out.push(first_value);
        current = rest_for(store, &current)?;
    }
    Ok(out)
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

/// Serialise an environment to its canonical Turtle form.
///
/// The byte layout is fixed so seed reproducibility (TC-055) does not
/// depend on an external serialiser's whims. Order of properties is
/// fixed: `a`, `dec:envType`, `dec:safetyClass`, `dec:allowedOps`,
/// `dec:setup`, `dec:teardown`, `dec:endpoint`.
#[must_use]
pub fn to_canonical_turtle(env: &VerificationEnvironment) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    out.push_str("# decision-cli VerificationEnvironment artifact.\n");
    out.push_str("# Authored by FT-035 / ADR-028. On-disk Turtle is authoritative.\n\n");
    out.push_str("@prefix dec: <https://decision-cli.dev/ns#> .\n\n");
    let _ = writeln!(out, "<{prefix}{id}>", prefix = IRI_DEC_ENV_PREFIX, id = env.id);
    out.push_str("    a dec:VerificationEnvironment ;\n");
    let _ = writeln!(
        out,
        "    dec:envType {value} ;",
        value = turtle_string(&env.env_type)
    );
    let _ = writeln!(
        out,
        "    dec:safetyClass {value} ;",
        value = turtle_string(env.safety_class.as_str())
    );
    let ops_list = format_allowed_ops_list(&env.allowed_ops);
    let _ = writeln!(out, "    dec:allowedOps {ops_list} ;");
    if let Some(s) = &env.setup {
        let _ = writeln!(out, "    dec:setup {value} ;", value = turtle_string(s));
    }
    if let Some(s) = &env.teardown {
        let _ = writeln!(
            out,
            "    dec:teardown {value} ;",
            value = turtle_string(s)
        );
    }
    if let Some(s) = &env.endpoint {
        let _ = writeln!(
            out,
            "    dec:endpoint {value} ;",
            value = turtle_string(s)
        );
    }
    // Replace the final ";" with "." for valid Turtle.
    if out.ends_with(" ;\n") {
        out.truncate(out.len() - 3);
        out.push_str(" .\n");
    }
    out
}

fn format_allowed_ops_list(ops: &[String]) -> String {
    if ops.is_empty() {
        return "()".to_string();
    }
    let mut parts: Vec<String> = ops.iter().map(|op| turtle_string(op)).collect();
    let mut out = String::from("( ");
    out.push_str(&parts.join(" "));
    out.push_str(" )");
    // discard `parts` reference to satisfy clippy if needed.
    let _ = &mut parts;
    out
}

fn turtle_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
