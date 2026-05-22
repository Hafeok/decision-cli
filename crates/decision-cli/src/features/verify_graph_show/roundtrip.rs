//! Reconstruct a `VerificationGraph` from a `GraphDocument` (FT-043 AC #5).
//!
//! Used by the TC-065 round-trip assertion: take the JSON the show
//! handler emits, parse it back into a `GraphDocument`, project it
//! through this module to a `VerificationGraph`, and serialise that
//! through `to_canonical_turtle`. The result must match the on-disk
//! Turtle byte-for-byte.

use oxigraph::model::NamedNode;

use crate::core::ontology::verification_graph::{
    step_iri_for, ArtifactRef, StepFields, VerificationGraph, VerificationStep,
};
use crate::core::vocab::{IRI_DEC_ENV_PREFIX, IRI_DEC_VERIFY_GRAPH_PREFIX};

use super::document::{GraphDocument, StepDocument};

/// IRI prefix for feature artifacts the graph's `dec:verifies` references.
const IRI_FEATURE_PREFIX: &str = "https://decision-cli.dev/ns/feature/";
/// IRI prefix for test-criterion artifacts the graph's `dec:verifies` references.
const IRI_TC_PREFIX: &str = "https://decision-cli.dev/ns/tc/";

/// Failures produced by [`document_to_graph`].
#[derive(Debug)]
pub enum ReconstructError {
    /// The document contained a malformed IRI fragment.
    Malformed(String),
}

impl std::fmt::Display for ReconstructError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(detail) => write!(f, "malformed graph document: {detail}"),
        }
    }
}

impl std::error::Error for ReconstructError {}

/// Reconstruct an in-memory [`VerificationGraph`] from its on-the-wire
/// document. IRI prefixes are reapplied verbatim per ADR-028.
pub fn document_to_graph(doc: &GraphDocument) -> Result<VerificationGraph, ReconstructError> {
    let graph_iri = build_prefixed_iri(IRI_DEC_VERIFY_GRAPH_PREFIX, &doc.id, "graph IRI")?;
    let verifies_iri = restore_verifies_iri(&doc.verifies)?;
    let environment_iri =
        build_prefixed_iri(IRI_DEC_ENV_PREFIX, &doc.environment, "environment IRI")?;
    let steps = reconstruct_steps(doc)?;
    Ok(VerificationGraph {
        id: graph_iri,
        verifies: ArtifactRef(verifies_iri),
        environment: environment_iri,
        steps,
    })
}

fn build_prefixed_iri(prefix: &str, id: &str, label: &str) -> Result<NamedNode, ReconstructError> {
    NamedNode::new(format!("{prefix}{id}"))
        .map_err(|e| ReconstructError::Malformed(format!("{label}: {e}")))
}

fn reconstruct_steps(doc: &GraphDocument) -> Result<Vec<VerificationStep>, ReconstructError> {
    let mut steps: Vec<VerificationStep> = Vec::with_capacity(doc.steps.len());
    for (index, step_doc) in doc.steps.iter().enumerate() {
        steps.push(reconstruct_one_step(&doc.id, index, step_doc)?);
    }
    Ok(steps)
}

fn reconstruct_one_step(
    graph_id: &str,
    index: usize,
    step_doc: &StepDocument,
) -> Result<VerificationStep, ReconstructError> {
    let id = step_iri_for(graph_id, index);
    let fields = step_fields_from_doc(step_doc)?;
    let provides_evidence_for = step_doc
        .provides_evidence_for()
        .iter()
        .map(|s| NamedNode::new(s.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ReconstructError::Malformed(format!("evidence IRI: {e}")))?;
    let kind = step_doc.kind_from_doc();
    Ok(VerificationStep {
        id,
        kind,
        fields,
        provides_evidence_for,
    })
}

/// Reverse of `canonicalize_verifies` — restore the IRI from a short id.
fn restore_verifies_iri(verifies: &str) -> Result<NamedNode, ReconstructError> {
    let iri = if verifies.starts_with("FT-") {
        format!("{IRI_FEATURE_PREFIX}{verifies}")
    } else if verifies.starts_with("TC-") {
        format!("{IRI_TC_PREFIX}{verifies}")
    } else {
        verifies.to_string()
    };
    NamedNode::new(iri).map_err(|e| ReconstructError::Malformed(format!("verifies IRI: {e}")))
}

fn step_fields_from_doc(doc: &StepDocument) -> Result<StepFields, ReconstructError> {
    match doc {
        StepDocument::WaitFor {
            condition, timeout, ..
        } => wait_for_fields(condition, timeout),
        StepDocument::Capture {
            bind_as, from_step, ..
        } => capture_fields(bind_as, from_step.as_deref()),
        other => Ok(infallible_fields_from_doc(other)),
    }
}

fn infallible_fields_from_doc(doc: &StepDocument) -> StepFields {
    match doc {
        StepDocument::ShellCommand {
            command,
            expect_exit_code,
            capture_output,
            ..
        } => shell_fields(command, *expect_exit_code, *capture_output),
        StepDocument::SparqlAssertion {
            target,
            query,
            expect_rows,
            ..
        } => sparql_fields(target, query, *expect_rows),
        StepDocument::FileAssertion {
            path,
            expect_hash,
            expect_content,
            ..
        } => file_fields(path, expect_hash.as_deref(), expect_content.as_deref()),
        StepDocument::HttpRequest {
            method,
            url,
            expect_status,
            ..
        } => http_fields(method, url, *expect_status),
        StepDocument::WaitFor { .. } | StepDocument::Capture { .. } => {
            unreachable!("fallible variants are handled by step_fields_from_doc")
        }
    }
}

fn shell_fields(
    command: &str,
    expect_exit_code: Option<i64>,
    capture_output: Option<bool>,
) -> StepFields {
    StepFields::ShellCommand {
        command: command.to_string(),
        expect_exit_code,
        capture_output,
    }
}

fn sparql_fields(target: &str, query: &str, expect_rows: Option<i64>) -> StepFields {
    StepFields::SparqlAssertion {
        target: target.to_string(),
        query: query.to_string(),
        expect_rows,
    }
}

fn file_fields(path: &str, expect_hash: Option<&str>, expect_content: Option<&str>) -> StepFields {
    StepFields::FileAssertion {
        path: path.to_string(),
        expect_hash: expect_hash.map(str::to_string),
        expect_content: expect_content.map(str::to_string),
    }
}

fn http_fields(method: &str, url: &str, expect_status: Option<i64>) -> StepFields {
    StepFields::HttpRequest {
        method: method.to_string(),
        url: url.to_string(),
        expect_status,
    }
}

fn wait_for_fields(condition: &str, timeout: &str) -> Result<StepFields, ReconstructError> {
    let cond = NamedNode::new(condition.to_string())
        .map_err(|e| ReconstructError::Malformed(format!("wait-for condition IRI: {e}")))?;
    Ok(StepFields::WaitFor {
        condition: cond,
        timeout: timeout.to_string(),
    })
}

fn capture_fields(bind_as: &str, from_step: Option<&str>) -> Result<StepFields, ReconstructError> {
    let from = match from_step {
        Some(s) => Some(
            NamedNode::new(s.to_string())
                .map_err(|e| ReconstructError::Malformed(format!("capture from_step IRI: {e}")))?,
        ),
        None => None,
    };
    Ok(StepFields::Capture {
        from_step: from,
        bind_as: bind_as.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_shell_command_doc() {
        let doc = GraphDocument {
            id: "VG-001".to_string(),
            verifies: "FT-001".to_string(),
            environment: "ENV-001-ephemeral-cli".to_string(),
            steps: vec![StepDocument::ShellCommand {
                command: "true".to_string(),
                expect_exit_code: Some(0),
                capture_output: None,
                provides_evidence_for: Vec::new(),
            }],
        };
        let graph = document_to_graph(&doc).expect("ok");
        assert_eq!(graph.steps.len(), 1);
        match &graph.steps[0].fields {
            StepFields::ShellCommand { command, .. } => assert_eq!(command, "true"),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
