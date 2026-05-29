//! On-the-wire graph document for `dec verify graph show` (FT-043).
//!
//! The [`GraphDocument`] mirrors a single `dec:VerificationGraph` with
//! its ordered step list. Optional step fields are omitted (not `null`)
//! so the JSON output stays compact and round-trips back to canonical
//! Turtle without reintroducing default values.

use oxigraph::model::NamedNode;
use serde::{Deserialize, Serialize};

use crate::core::ontology::verification_graph::{StepFields, VerificationGraph};
use crate::core::vocab::{IRI_DEC_BENCH_PREFIX, IRI_DEC_VERIFY_GRAPH_PREFIX};

/// IRI prefix for feature artifacts the graph's `dec:verifies` references.
const IRI_FEATURE_PREFIX: &str = "https://decision-cli.dev/ns/feature/";
/// IRI prefix for test-criterion artifacts the graph's `dec:verifies` references.
const IRI_TC_PREFIX: &str = "https://decision-cli.dev/ns/tc/";

/// Per-step document — discriminated by `kind`. Optional fields are
/// omitted when absent so the JSON output stays compact and round-trips
/// to canonical Turtle without re-introducing default values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StepDocument {
    /// `shell-command` step.
    ShellCommand {
        /// Command text (may contain `${name}` placeholders).
        command: String,
        /// Expected exit code; omitted when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect_exit_code: Option<i64>,
        /// Whether stdout is bound for downstream `capture` steps.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        capture_output: Option<bool>,
        /// TC IRIs this step provides evidence for.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        provides_evidence_for: Vec<String>,
    },
    /// `sparql-assertion` step.
    SparqlAssertion {
        /// SPARQL endpoint or local store path.
        target: String,
        /// Query text.
        query: String,
        /// Expected row count; omitted when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect_rows: Option<i64>,
        /// TC IRIs this step provides evidence for.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        provides_evidence_for: Vec<String>,
    },
    /// `file-assertion` step.
    FileAssertion {
        /// Target file path.
        path: String,
        /// Expected content hash; omitted when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect_hash: Option<String>,
        /// Expected content; omitted when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect_content: Option<String>,
        /// TC IRIs this step provides evidence for.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        provides_evidence_for: Vec<String>,
    },
    /// `http-request` step.
    HttpRequest {
        /// HTTP method (`GET`, `POST`, …).
        method: String,
        /// Target URL (may contain `${name}` placeholders).
        url: String,
        /// Expected status code; omitted when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect_status: Option<i64>,
        /// TC IRIs this step provides evidence for.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        provides_evidence_for: Vec<String>,
    },
    /// `wait-for` step.
    WaitFor {
        /// Sub-condition reference (typically a step IRI).
        condition: String,
        /// ISO 8601 timeout duration.
        timeout: String,
        /// TC IRIs this step provides evidence for.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        provides_evidence_for: Vec<String>,
    },
    /// `capture` step.
    Capture {
        /// Binding name (e.g. `manifest_sha`).
        bind_as: String,
        /// Optional source step IRI the capture binds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_step: Option<String>,
        /// TC IRIs this step provides evidence for.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        provides_evidence_for: Vec<String>,
    },
}

impl StepDocument {
    /// Stable wire string for the step kind.
    #[must_use]
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::ShellCommand { .. } => "shell-command",
            Self::SparqlAssertion { .. } => "sparql-assertion",
            Self::FileAssertion { .. } => "file-assertion",
            Self::HttpRequest { .. } => "http-request",
            Self::WaitFor { .. } => "wait-for",
            Self::Capture { .. } => "capture",
        }
    }

    /// Borrow the per-step `dec:providesEvidenceFor` list verbatim.
    #[must_use]
    pub fn provides_evidence_for(&self) -> &[String] {
        match self {
            Self::ShellCommand {
                provides_evidence_for,
                ..
            }
            | Self::SparqlAssertion {
                provides_evidence_for,
                ..
            }
            | Self::FileAssertion {
                provides_evidence_for,
                ..
            }
            | Self::HttpRequest {
                provides_evidence_for,
                ..
            }
            | Self::WaitFor {
                provides_evidence_for,
                ..
            }
            | Self::Capture {
                provides_evidence_for,
                ..
            } => provides_evidence_for,
        }
    }

    /// Map the document variant back to the in-memory `StepKind` tag.
    /// Used by the round-trip path to rebuild an in-memory step.
    #[must_use]
    pub fn kind_from_doc(&self) -> crate::core::ontology::verification_graph::StepKind {
        use crate::core::ontology::verification_graph::StepKind;
        match self {
            Self::ShellCommand { .. } => StepKind::ShellCommand,
            Self::SparqlAssertion { .. } => StepKind::SparqlAssertion,
            Self::FileAssertion { .. } => StepKind::FileAssertion,
            Self::HttpRequest { .. } => StepKind::HttpRequest,
            Self::WaitFor { .. } => StepKind::WaitFor,
            Self::Capture { .. } => StepKind::Capture,
        }
    }

    /// Project an in-memory step's discriminated fields plus its evidence
    /// list into the on-the-wire document.
    pub(super) fn from_fields(fields: &StepFields, evidence: &[NamedNode]) -> Self {
        let provides = evidence_to_strings(evidence);
        dispatch_from_fields(fields, provides)
    }
}

fn evidence_to_strings(evidence: &[NamedNode]) -> Vec<String> {
    evidence.iter().map(|n| n.as_str().to_string()).collect()
}

fn dispatch_from_fields(fields: &StepFields, provides: Vec<String>) -> StepDocument {
    match fields {
        StepFields::WaitFor { condition, timeout } => {
            wait_for_doc(condition.as_str(), timeout, provides)
        }
        StepFields::Capture { from_step, bind_as } => {
            capture_doc(bind_as, from_step.as_ref().map(NamedNode::as_str), provides)
        }
        other => dispatch_assertion_fields(other, provides),
    }
}

fn dispatch_assertion_fields(fields: &StepFields, provides: Vec<String>) -> StepDocument {
    match fields {
        StepFields::ShellCommand {
            command,
            expect_exit_code,
            capture_output,
        } => shell_command_doc(command, *expect_exit_code, *capture_output, provides),
        StepFields::SparqlAssertion {
            target,
            query,
            expect_rows,
        } => sparql_assertion_doc(target, query, *expect_rows, provides),
        other => dispatch_remote_assertion_fields(other, provides),
    }
}

fn dispatch_remote_assertion_fields(fields: &StepFields, provides: Vec<String>) -> StepDocument {
    match fields {
        StepFields::FileAssertion {
            path,
            expect_hash,
            expect_content,
        } => file_assertion_doc(
            path,
            expect_hash.as_deref(),
            expect_content.as_deref(),
            provides,
        ),
        StepFields::HttpRequest {
            method,
            url,
            expect_status,
        } => http_request_doc(method, url, *expect_status, provides),
        _ => unreachable!("handled by dispatch_from_fields and dispatch_assertion_fields"),
    }
}

fn shell_command_doc(
    command: &str,
    expect_exit_code: Option<i64>,
    capture_output: Option<bool>,
    provides: Vec<String>,
) -> StepDocument {
    StepDocument::ShellCommand {
        command: command.to_string(),
        expect_exit_code,
        capture_output,
        provides_evidence_for: provides,
    }
}

fn sparql_assertion_doc(
    target: &str,
    query: &str,
    expect_rows: Option<i64>,
    provides: Vec<String>,
) -> StepDocument {
    StepDocument::SparqlAssertion {
        target: target.to_string(),
        query: query.to_string(),
        expect_rows,
        provides_evidence_for: provides,
    }
}

fn file_assertion_doc(
    path: &str,
    expect_hash: Option<&str>,
    expect_content: Option<&str>,
    provides: Vec<String>,
) -> StepDocument {
    StepDocument::FileAssertion {
        path: path.to_string(),
        expect_hash: expect_hash.map(str::to_string),
        expect_content: expect_content.map(str::to_string),
        provides_evidence_for: provides,
    }
}

fn http_request_doc(
    method: &str,
    url: &str,
    expect_status: Option<i64>,
    provides: Vec<String>,
) -> StepDocument {
    StepDocument::HttpRequest {
        method: method.to_string(),
        url: url.to_string(),
        expect_status,
        provides_evidence_for: provides,
    }
}

fn wait_for_doc(condition: &str, timeout: &str, provides: Vec<String>) -> StepDocument {
    StepDocument::WaitFor {
        condition: condition.to_string(),
        timeout: timeout.to_string(),
        provides_evidence_for: provides,
    }
}

fn capture_doc(bind_as: &str, from_step: Option<&str>, provides: Vec<String>) -> StepDocument {
    StepDocument::Capture {
        bind_as: bind_as.to_string(),
        from_step: from_step.map(str::to_string),
        provides_evidence_for: provides,
    }
}

/// Full graph document — `id`, `verifies`, `environment`, and the ordered
/// step list. Mirrors FT-043 §Outputs `--format json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphDocument {
    /// `VG-NNN[-suffix]` identifier.
    pub id: String,
    /// Canonical short id of the artifact this graph verifies
    /// (`FT-NNN` or `TC-NNN`). Unknown IRI prefixes pass through verbatim.
    pub verifies: String,
    /// Canonical short id of the environment the graph executes against
    /// (e.g. `BNCH-001-ephemeral-cli`). Unknown IRI prefixes pass through.
    pub environment: String,
    /// Ordered step documents in `dec:steps` rdf:List order.
    pub steps: Vec<StepDocument>,
}

impl GraphDocument {
    /// Project a [`VerificationGraph`] into the on-the-wire document.
    #[must_use]
    pub fn from_graph(graph: &VerificationGraph) -> Self {
        let id = graph
            .id
            .as_str()
            .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
            .unwrap_or(graph.id.as_str())
            .to_string();
        let verifies = canonicalize_verifies(graph.verifies.0.as_str());
        let environment = graph
            .environment
            .as_str()
            .strip_prefix(IRI_DEC_BENCH_PREFIX)
            .unwrap_or(graph.environment.as_str())
            .to_string();
        let steps = graph
            .steps
            .iter()
            .map(|s| StepDocument::from_fields(&s.fields, &s.provides_evidence_for))
            .collect();
        Self {
            id,
            verifies,
            environment,
            steps,
        }
    }
}

/// Strip the feature/TC IRI prefix so callers receive `FT-001` / `TC-013`.
/// Unknown prefixes pass through unchanged.
pub(super) fn canonicalize_verifies(iri: &str) -> String {
    if let Some(tail) = iri.strip_prefix(IRI_FEATURE_PREFIX) {
        return tail.to_string();
    }
    if let Some(tail) = iri.strip_prefix(IRI_TC_PREFIX) {
        return tail.to_string();
    }
    iri.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_verifies_strips_feature_prefix() {
        assert_eq!(
            canonicalize_verifies("https://decision-cli.dev/ns/feature/FT-007"),
            "FT-007"
        );
    }

    #[test]
    fn canonicalize_verifies_strips_tc_prefix() {
        assert_eq!(
            canonicalize_verifies("https://decision-cli.dev/ns/tc/TC-013"),
            "TC-013"
        );
    }

    #[test]
    fn canonicalize_verifies_passes_through_unknown() {
        assert_eq!(
            canonicalize_verifies("https://example.com/other"),
            "https://example.com/other"
        );
    }

    #[test]
    fn step_document_kind_str_matches_variants() {
        let d = StepDocument::Capture {
            bind_as: "x".to_string(),
            from_step: None,
            provides_evidence_for: Vec::new(),
        };
        assert_eq!(d.kind_str(), "capture");
    }
}
