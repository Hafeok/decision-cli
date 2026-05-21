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
    let graph_iri = NamedNode::new(format!(
        "{prefix}{id}",
        prefix = IRI_DEC_VERIFY_GRAPH_PREFIX,
        id = doc.id
    ))
    .map_err(|e| ReconstructError::Malformed(format!("graph IRI: {e}")))?;
    let verifies_iri = restore_verifies_iri(&doc.verifies)?;
    let environment_iri = NamedNode::new(format!(
        "{prefix}{id}",
        prefix = IRI_DEC_ENV_PREFIX,
        id = doc.environment
    ))
    .map_err(|e| ReconstructError::Malformed(format!("environment IRI: {e}")))?;
    let mut steps: Vec<VerificationStep> = Vec::with_capacity(doc.steps.len());
    for (index, step_doc) in doc.steps.iter().enumerate() {
        let id = step_iri_for(&doc.id, index);
        let fields = step_fields_from_doc(step_doc)?;
        let provides_evidence_for = step_doc
            .provides_evidence_for()
            .iter()
            .map(|s| NamedNode::new(s.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ReconstructError::Malformed(format!("evidence IRI: {e}")))?;
        let kind = step_doc.kind_from_doc();
        steps.push(VerificationStep {
            id,
            kind,
            fields,
            provides_evidence_for,
        });
    }
    Ok(VerificationGraph {
        id: graph_iri,
        verifies: ArtifactRef(verifies_iri),
        environment: environment_iri,
        steps,
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
    Ok(match doc {
        StepDocument::ShellCommand {
            command,
            expect_exit_code,
            capture_output,
            ..
        } => StepFields::ShellCommand {
            command: command.clone(),
            expect_exit_code: *expect_exit_code,
            capture_output: *capture_output,
        },
        StepDocument::SparqlAssertion {
            target,
            query,
            expect_rows,
            ..
        } => StepFields::SparqlAssertion {
            target: target.clone(),
            query: query.clone(),
            expect_rows: *expect_rows,
        },
        StepDocument::FileAssertion {
            path,
            expect_hash,
            expect_content,
            ..
        } => StepFields::FileAssertion {
            path: path.clone(),
            expect_hash: expect_hash.clone(),
            expect_content: expect_content.clone(),
        },
        StepDocument::HttpRequest {
            method,
            url,
            expect_status,
            ..
        } => StepFields::HttpRequest {
            method: method.clone(),
            url: url.clone(),
            expect_status: *expect_status,
        },
        StepDocument::WaitFor {
            condition, timeout, ..
        } => {
            let cond = NamedNode::new(condition.clone()).map_err(|e| {
                ReconstructError::Malformed(format!("wait-for condition IRI: {e}"))
            })?;
            StepFields::WaitFor {
                condition: cond,
                timeout: timeout.clone(),
            }
        }
        StepDocument::Capture {
            bind_as, from_step, ..
        } => {
            let from = match from_step.as_ref() {
                Some(s) => Some(NamedNode::new(s.clone()).map_err(|e| {
                    ReconstructError::Malformed(format!("capture from_step IRI: {e}"))
                })?),
                None => None,
            };
            StepFields::Capture {
                from_step: from,
                bind_as: bind_as.clone(),
            }
        }
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
