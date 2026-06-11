//! In-memory `VerificationGraph` + `VerificationStep` types (ADR-028).
//!
//! Serialisation to RDF quads lives in the sibling [`super::quads`]
//! module; this file is intentionally schema-only.

use oxrdf::NamedNode;

use crate::vocab::{
    IRI_DEC_STEP_PREFIX, IRI_DEC_VERIFY_GRAPH_PREFIX, STEP_KIND_CAPTURE, STEP_KIND_FILE_ASSERTION,
    STEP_KIND_HTTP_REQUEST, STEP_KIND_SHELL_COMMAND, STEP_KIND_SPARQL_ASSERTION,
    STEP_KIND_WAIT_FOR,
};

/// IRI of `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
/// IRI of `rdf:first` (RDF collection head).
pub const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
/// IRI of `rdf:rest` (RDF collection tail).
pub const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
/// IRI of `rdf:nil` (RDF collection terminator).
pub const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// Alias for the verification graph's IRI (e.g. `…/ns/graph/VG-001`).
pub type GraphIri = NamedNode;
/// Alias for a step IRI (e.g. `…/ns/step/VG-001/0`).
pub type StepIri = NamedNode;

/// Polymorphic `dec:verifies` reference — a feature or a TC (ADR-028).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactRef(pub NamedNode);

impl ArtifactRef {
    /// Borrow as a `NamedNode`.
    #[must_use]
    pub fn as_named(&self) -> &NamedNode {
        &self.0
    }
}

/// Seed step kind tags per ADR-028.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepKind {
    /// Run a shell command; assert exit code and/or stdout pattern.
    ShellCommand,
    /// Run a SPARQL query; assert row count or specific values.
    SparqlAssertion,
    /// Assert file existence, content equality, or hash.
    FileAssertion,
    /// Make an HTTP call; assert status and response shape.
    HttpRequest,
    /// Poll a sub-condition with timeout.
    WaitFor,
    /// Bind a prior step's stdout/result to a name.
    Capture,
}

impl StepKind {
    /// Stable wire string used as the `dec:stepType` literal value.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ShellCommand => STEP_KIND_SHELL_COMMAND,
            Self::SparqlAssertion => STEP_KIND_SPARQL_ASSERTION,
            Self::FileAssertion => STEP_KIND_FILE_ASSERTION,
            Self::HttpRequest => STEP_KIND_HTTP_REQUEST,
            Self::WaitFor => STEP_KIND_WAIT_FOR,
            Self::Capture => STEP_KIND_CAPTURE,
        }
    }

    /// Parse a step kind from its wire string. Returns `None` for unknown
    /// inputs; callers surface an `UnknownStepKindError`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            STEP_KIND_SHELL_COMMAND => Some(Self::ShellCommand),
            STEP_KIND_SPARQL_ASSERTION => Some(Self::SparqlAssertion),
            STEP_KIND_FILE_ASSERTION => Some(Self::FileAssertion),
            STEP_KIND_HTTP_REQUEST => Some(Self::HttpRequest),
            STEP_KIND_WAIT_FOR => Some(Self::WaitFor),
            STEP_KIND_CAPTURE => Some(Self::Capture),
            _ => None,
        }
    }
}

/// Discriminated union of per-kind step fields (ADR-028).
///
/// Optional fields use `Option`. `${name}` placeholders are preserved
/// verbatim — FT-036 does not interpret them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepFields {
    /// `shell-command` — `dec:command` + optional `dec:expectExitCode` and `dec:captureOutput`.
    ShellCommand {
        /// Command text. `${name}` placeholders preserved verbatim.
        command: String,
        /// Expected exit code; `None` ⇒ no assertion.
        expect_exit_code: Option<i64>,
        /// If true, stdout is bound for downstream `capture` steps.
        capture_output: Option<bool>,
    },
    /// `sparql-assertion` — `dec:target` + `dec:query` + optional `dec:expectRows`.
    SparqlAssertion {
        /// SPARQL endpoint or local store path.
        target: String,
        /// Query text.
        query: String,
        /// Expected row count; `None` ⇒ no assertion.
        expect_rows: Option<i64>,
    },
    /// `file-assertion` — `dec:path` + optional `dec:expectHash` / `dec:expectContent`.
    FileAssertion {
        /// Target file path.
        path: String,
        /// Expected content hash.
        expect_hash: Option<String>,
        /// Expected content.
        expect_content: Option<String>,
    },
    /// `http-request` — `dec:method` + `dec:url` + optional `dec:expectStatus`.
    HttpRequest {
        /// HTTP method (e.g. `GET`, `POST`).
        method: String,
        /// Target URL.
        url: String,
        /// Expected status code; `None` ⇒ no assertion.
        expect_status: Option<i64>,
    },
    /// `wait-for` — `dec:condition` (IRI) + `dec:timeout` (ISO 8601 duration).
    WaitFor {
        /// Sub-condition reference (typically a step IRI).
        condition: NamedNode,
        /// ISO 8601 timeout duration (e.g. `PT10S`).
        timeout: String,
    },
    /// `capture` — `dec:bindAs` + optional `dec:fromStep`.
    Capture {
        /// Source step the capture binds. Optional in slice 2.5 authoring.
        from_step: Option<NamedNode>,
        /// Binding name (e.g. `manifest_sha`).
        bind_as: String,
    },
}

impl StepFields {
    /// The kind tag corresponding to this fields variant.
    #[must_use]
    pub const fn kind(&self) -> StepKind {
        match self {
            Self::ShellCommand { .. } => StepKind::ShellCommand,
            Self::SparqlAssertion { .. } => StepKind::SparqlAssertion,
            Self::FileAssertion { .. } => StepKind::FileAssertion,
            Self::HttpRequest { .. } => StepKind::HttpRequest,
            Self::WaitFor { .. } => StepKind::WaitFor,
            Self::Capture { .. } => StepKind::Capture,
        }
    }
}

/// In-memory step artifact. IRIs are deterministic per `(graph_id, index)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationStep {
    /// Stable IRI per ADR-028 IRI scheme.
    pub id: StepIri,
    /// Kind tag (mirrors `fields.kind()`).
    pub kind: StepKind,
    /// Discriminated-union body.
    pub fields: StepFields,
    /// Optional, multi-valued coverage predicate (TC IRIs).
    pub provides_evidence_for: Vec<NamedNode>,
}

impl VerificationStep {
    /// Construct a step with `id` derived from `(graph_id, index)`.
    #[must_use]
    pub fn new(graph_id: &str, index: usize, fields: StepFields) -> Self {
        let id = step_iri_for(graph_id, index);
        let kind = fields.kind();
        Self {
            id,
            kind,
            fields,
            provides_evidence_for: Vec::new(),
        }
    }
}

/// In-memory `dec:VerificationGraph` artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationGraph {
    /// Stable IRI per ADR-028 IRI scheme (`…/ns/graph/VG-NNN`).
    pub id: GraphIri,
    /// The artifact the graph proves about — feature or TC (polymorphic).
    pub verifies: ArtifactRef,
    /// The VerificationBench this graph executes against.
    pub environment: NamedNode,
    /// Ordered steps. May be empty during authoring (FT-036 §Outputs).
    pub steps: Vec<VerificationStep>,
}

impl VerificationGraph {
    /// Construct a graph from a canonical id string (e.g. `VG-001`).
    #[must_use]
    pub fn new(
        id_str: &str,
        verifies: ArtifactRef,
        environment: NamedNode,
        steps: Vec<VerificationStep>,
    ) -> Self {
        Self {
            id: graph_iri_for(id_str),
            verifies,
            environment,
            steps,
        }
    }

    /// Numeric/canonical `VG-NNN-…` id substring of the IRI.
    #[must_use]
    pub fn id_str(&self) -> &str {
        self.id
            .as_str()
            .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
            .unwrap_or(self.id.as_str())
    }
}

/// Build a graph IRI deterministically from a canonical id string.
#[must_use]
pub fn graph_iri_for(id_str: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{IRI_DEC_VERIFY_GRAPH_PREFIX}{id_str}"))
}

/// Build a step IRI deterministically from `(graph_id, index)`.
#[must_use]
pub fn step_iri_for(graph_id: &str, index: usize) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "{prefix}{graph_id}/{index}",
        prefix = IRI_DEC_STEP_PREFIX,
    ))
}
