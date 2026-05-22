//! Field-level Turtle parsing helpers for `VerificationStep` (FT-036).
//!
//! Pure read-side primitives over an Oxigraph staging store. Exported to
//! the sibling [`super::io`] module which drives the top-level extraction.

use std::path::Path;

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use crate::core::vocab::{
    IRI_DEC_BIND_AS, IRI_DEC_CAPTURE_OUTPUT, IRI_DEC_COMMAND, IRI_DEC_CONDITION,
    IRI_DEC_EXPECT_CONTENT, IRI_DEC_EXPECT_EXIT_CODE, IRI_DEC_EXPECT_HASH, IRI_DEC_EXPECT_ROWS,
    IRI_DEC_EXPECT_STATUS, IRI_DEC_FROM_STEP, IRI_DEC_METHOD, IRI_DEC_PATH, IRI_DEC_QUERY,
    IRI_DEC_TARGET, IRI_DEC_TIMEOUT, IRI_DEC_URL,
};

use super::io::GraphIoError;
use super::types::{StepFields, StepKind};

pub(super) fn parse_kind_fields(
    store: &Store,
    subject: &NamedNode,
    kind: StepKind,
    path: &Path,
) -> Result<StepFields, GraphIoError> {
    match kind {
        StepKind::ShellCommand => parse_shell_command(store, subject, path),
        StepKind::SparqlAssertion => parse_sparql_assertion(store, subject, path),
        StepKind::FileAssertion => parse_file_assertion(store, subject, path),
        StepKind::HttpRequest => parse_http_request(store, subject, path),
        StepKind::WaitFor => parse_wait_for(store, subject, path),
        StepKind::Capture => parse_capture(store, subject, path),
    }
}

fn parse_shell_command(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<StepFields, GraphIoError> {
    let command = single_literal(store, subject, IRI_DEC_COMMAND, path)?
        .ok_or_else(|| missing(path, subject, "dec:command"))?;
    let expect_exit_code = single_integer(store, subject, IRI_DEC_EXPECT_EXIT_CODE, path)?;
    let capture_output = single_boolean(store, subject, IRI_DEC_CAPTURE_OUTPUT, path)?;
    Ok(StepFields::ShellCommand {
        command,
        expect_exit_code,
        capture_output,
    })
}

fn parse_sparql_assertion(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<StepFields, GraphIoError> {
    let target = single_literal(store, subject, IRI_DEC_TARGET, path)?
        .ok_or_else(|| missing(path, subject, "dec:target"))?;
    let query = single_literal(store, subject, IRI_DEC_QUERY, path)?
        .ok_or_else(|| missing(path, subject, "dec:query"))?;
    let expect_rows = single_integer(store, subject, IRI_DEC_EXPECT_ROWS, path)?;
    Ok(StepFields::SparqlAssertion {
        target,
        query,
        expect_rows,
    })
}

fn parse_file_assertion(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<StepFields, GraphIoError> {
    let p = single_literal(store, subject, IRI_DEC_PATH, path)?
        .ok_or_else(|| missing(path, subject, "dec:path"))?;
    let expect_hash = single_literal(store, subject, IRI_DEC_EXPECT_HASH, path)?;
    let expect_content = single_literal(store, subject, IRI_DEC_EXPECT_CONTENT, path)?;
    Ok(StepFields::FileAssertion {
        path: p,
        expect_hash,
        expect_content,
    })
}

fn parse_http_request(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<StepFields, GraphIoError> {
    let method = single_literal(store, subject, IRI_DEC_METHOD, path)?
        .ok_or_else(|| missing(path, subject, "dec:method"))?;
    let url = single_literal(store, subject, IRI_DEC_URL, path)?
        .ok_or_else(|| missing(path, subject, "dec:url"))?;
    let expect_status = single_integer(store, subject, IRI_DEC_EXPECT_STATUS, path)?;
    Ok(StepFields::HttpRequest {
        method,
        url,
        expect_status,
    })
}

fn parse_wait_for(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<StepFields, GraphIoError> {
    let condition = single_iri(store, subject, IRI_DEC_CONDITION, path)?
        .ok_or_else(|| missing(path, subject, "dec:condition"))?;
    let timeout = single_literal(store, subject, IRI_DEC_TIMEOUT, path)?
        .ok_or_else(|| missing(path, subject, "dec:timeout"))?;
    Ok(StepFields::WaitFor { condition, timeout })
}

fn parse_capture(
    store: &Store,
    subject: &NamedNode,
    path: &Path,
) -> Result<StepFields, GraphIoError> {
    let bind_as = single_literal(store, subject, IRI_DEC_BIND_AS, path)?
        .ok_or_else(|| missing(path, subject, "dec:bindAs"))?;
    let from_step = single_iri(store, subject, IRI_DEC_FROM_STEP, path)?;
    Ok(StepFields::Capture { from_step, bind_as })
}

fn missing(path: &Path, subject: &NamedNode, field: &str) -> GraphIoError {
    GraphIoError::MalformedShape {
        path: path.to_path_buf(),
        detail: format!("step <{}> missing {}", subject.as_str(), field),
    }
}

pub(super) fn single_literal(
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
    path: &Path,
) -> Result<Option<String>, GraphIoError> {
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
        n => Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

pub(super) fn single_iri(
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
    path: &Path,
) -> Result<Option<NamedNode>, GraphIoError> {
    let mut out: Vec<NamedNode> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(subject.clone()).as_ref()),
            Some(NamedNode::new_unchecked(predicate).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::NamedNode(n) = quad.object {
            out.push(n);
        }
    }
    match out.len() {
        0 => Ok(None),
        1 => Ok(Some(out.remove(0))),
        n => Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

fn single_integer(
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
    path: &Path,
) -> Result<Option<i64>, GraphIoError> {
    let Some(raw) = single_literal(store, subject, predicate, path)? else {
        return Ok(None);
    };
    let parsed = raw
        .parse::<i64>()
        .map_err(|e| GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("{predicate} not parseable as integer: {e}"),
        })?;
    Ok(Some(parsed))
}

fn single_boolean(
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
    path: &Path,
) -> Result<Option<bool>, GraphIoError> {
    let Some(raw) = single_literal(store, subject, predicate, path)? else {
        return Ok(None);
    };
    match raw.as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => Err(GraphIoError::MalformedShape {
            path: path.to_path_buf(),
            detail: format!("{predicate} not parseable as boolean: {raw:?}"),
        }),
    }
}

pub(super) fn read_iri_list(store: &Store, subject: &NamedNode, predicate: &str) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(subject.clone()).as_ref()),
            Some(NamedNode::new_unchecked(predicate).as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::NamedNode(n) = quad.object {
            out.push(n);
        }
    }
    out
}
