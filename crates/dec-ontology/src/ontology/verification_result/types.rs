//! In-memory `VerificationGraphResult` + `VerificationStepTrace` types
//! (FT-097 / ADR-028).
//!
//! Serialisation to RDF quads lives in the sibling [`super::quads`]
//! module; this file is schema-only. The types are `serde`-serialisable
//! so callers can pass them across worker boundaries without re-deriving.

use oxrdf::NamedNode;

use crate::ontology::verdict::Verdict;
use crate::vocab::{IRI_DEC_VERIFY_RESULT_PREFIX, OUTCOME_FAIL, OUTCOME_PASS, OUTCOME_UNRUNNABLE};

/// IRI of `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
/// IRI of `rdf:first` (RDF collection head).
pub const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
/// IRI of `rdf:rest` (RDF collection tail).
pub const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
/// IRI of `rdf:nil` (RDF collection terminator).
pub const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// IRI of PROV-O `wasGeneratedBy`.
pub const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
/// IRI of PROV-O `wasAttributedTo`.
pub const PROV_WAS_ATTRIBUTED_TO: &str = "http://www.w3.org/ns/prov#wasAttributedTo";
/// IRI of `dcterms:created`.
pub const DCTERMS_CREATED: &str = "http://purl.org/dc/terms/created";

/// Wire token for a step's outcome (`pass` / `fail` / `unrunnable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepOutcome {
    /// The step's assertion(s) were satisfied.
    Pass,
    /// The step ran but its assertion(s) failed.
    Fail,
    /// The step could not be evaluated (timeout, missing target, runtime
    /// op-violation caught at runner-time).
    Unrunnable,
}

impl StepOutcome {
    /// Stable wire string (`pass` / `fail` / `unrunnable`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => OUTCOME_PASS,
            Self::Fail => OUTCOME_FAIL,
            Self::Unrunnable => OUTCOME_UNRUNNABLE,
        }
    }

    /// Parse from wire string. Returns `None` for unknown inputs.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            OUTCOME_PASS => Some(Self::Pass),
            OUTCOME_FAIL => Some(Self::Fail),
            OUTCOME_UNRUNNABLE => Some(Self::Unrunnable),
            _ => None,
        }
    }
}

/// Per-step trace, one per step run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationStepTrace {
    /// Stable IRI per ADR-028 (`…/ns/result/VGR-NNN/step/I`).
    pub id: String,
    /// IRI of the step in the parent `VerificationGraph` this trace records.
    pub traces_step: String,
    /// Two-tier outcome per ADR-013 / ADR-028.
    pub outcome: StepOutcome,
    /// UTC ISO 8601 start instant.
    pub started_at: String,
    /// UTC ISO 8601 end instant.
    pub ended_at: String,
    /// Populated for `shell-command` and `http-request`; `None` for `capture`.
    pub exit_code: Option<i64>,
    /// Cap-4-KiB excerpt of step's stdout. Empty string is valid.
    pub stdout_excerpt: String,
    /// Cap-4-KiB excerpt of step's stderr. Empty string is valid.
    pub stderr_excerpt: String,
    /// One-line summary, populated when `outcome != pass`.
    pub error_message: String,
    /// IRI of the run activity (`prov:wasGeneratedBy`).
    pub was_generated_by: String,
}

/// A derived `(TC, step-trace, outcome)` projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProjection {
    /// TC IRI receiving the evidence.
    pub tc: String,
    /// Outcome attributed to the TC for this run.
    pub outcome: StepOutcome,
    /// IRI of the step-trace that emitted the outcome.
    pub from_step: String,
}

/// In-memory `dec:VerificationGraphResult` artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationGraphResult {
    /// Stable IRI per ADR-028 (`…/ns/result/VGR-NNN`).
    pub id: String,
    /// IRI of the `VerificationGraph` this run executed.
    pub result_of: String,
    /// IRI of the `VerificationBench` this run ran in.
    pub ran_in_environment: String,
    /// Per-graph verdict.
    pub verdict: Verdict,
    /// UTC ISO 8601 envelope start.
    pub started_at: String,
    /// UTC ISO 8601 envelope end.
    pub ended_at: String,
    /// Ordered step traces. Matches parent graph step order element-for-element.
    pub step_traces: Vec<VerificationStepTrace>,
    /// Derived per-`(TC, step)` projections. May be empty.
    pub evidence_for: Vec<EvidenceProjection>,
    /// Operator-facing one-line rationale (≥ 20 chars).
    pub rationale: String,
    /// IRI of the run activity (`prov:wasGeneratedBy`).
    pub was_generated_by: String,
    /// IRI of the runner role agent (`prov:wasAttributedTo`).
    pub was_attributed_to: String,
    /// UTC ISO 8601 persistence timestamp.
    pub created_at: String,
}

impl VerificationGraphResult {
    /// Construct an IRI from a canonical id string (e.g. `VGR-001`).
    #[must_use]
    pub fn iri_for(id_str: &str) -> NamedNode {
        NamedNode::new_unchecked(format!("{IRI_DEC_VERIFY_RESULT_PREFIX}{id_str}"))
    }

    /// Canonical step-trace IRI for `(result-id, step-index)`.
    #[must_use]
    pub fn step_trace_iri_for(result_id: &str, index: usize) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{IRI_DEC_VERIFY_RESULT_PREFIX}{result_id}/step/{index}"
        ))
    }
}
