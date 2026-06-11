//! Read API for `Feedback` artifacts (FT-026 §Outputs).

use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use dec_ontology::vocab::{
    FEEDBACK_TERMINAL_STATES, IRI_DEC_ADDRESSING_ARTIFACT, IRI_DEC_CLOSED_BY,
    IRI_DEC_DISPOSITION_OVERRIDE, IRI_DEC_DISPOSITION_RATIONALE, IRI_DEC_EVIDENCE,
    IRI_DEC_FEEDBACK, IRI_DEC_FEEDBACK_CLASS, IRI_DEC_IN_STREAM, IRI_DEC_LIFECYCLE_STATE,
    IRI_DEC_RECEIVING_SESSION, IRI_DEC_RECOMMENDATION, IRI_DEC_REJECTION_REASON, IRI_DEC_ROUTED_AT,
    IRI_DEC_SEVERITY, IRI_DEC_SOURCE_ARTIFACT, IRI_DEC_SOURCE_SESSION, IRI_DEC_SUPERSEDED_BY,
    IRI_DEC_TARGET_ROLE,
};

use super::artifact::{Feedback, Severity};
use super::shacl::{iri_values, literal_values};

/// Errors produced by the read API.
#[derive(Debug, Error)]
pub enum FeedbackReadError {
    /// `get` against an IRI that has no `dec:Feedback` rdf:type triple.
    #[error("feedback not found: <{iri}>")]
    NotFound {
        /// The IRI looked up.
        iri: String,
    },
    /// Store contains a feedback artifact whose RDF shape contradicts the schema.
    #[error("malformed feedback <{iri}>: {detail}")]
    Malformed {
        /// IRI of the offending feedback subject.
        iri: String,
        /// Why the shape failed.
        detail: String,
    },
    /// Oxigraph store-level failure.
    #[error("oxigraph store error: {0}")]
    Store(String),
}

/// Resolve a single feedback artifact by IRI. Returns `NotFound` if no
/// `dec:Feedback` rdf:type triple exists for `iri`.
pub fn get(store: &Store, iri: &NamedNode) -> Result<Feedback, FeedbackReadError> {
    let quads = collect_subject_quads(store, iri)?;
    if !has_feedback_type(&quads, iri) {
        return Err(FeedbackReadError::NotFound {
            iri: iri.as_str().to_string(),
        });
    }
    parse_feedback(iri, &quads)
}

/// Return every open feedback (lifecycle state ∉ closed/rejected/superseded
/// AND no routing-time `dec:rejectionReason` recorded) scoped to the
/// given stream IRI. Order is by IRI lexicographic ascending.
///
/// The routing handler (FT-029) records `dec:rejectionReason` directly
/// on feedback whose routing failed without progressing the lifecycle
/// out of `produced`. Those rows are equivalent to terminal for the
/// "still-open queue" — filter them here.
pub fn list_open(store: &Store, stream: &NamedNode) -> Result<Vec<Feedback>, FeedbackReadError> {
    let iris = list_feedback_iris(store, Some(stream))?;
    let mut out: Vec<Feedback> = Vec::with_capacity(iris.len());
    for iri in iris {
        let node = NamedNode::new(&iri).map_err(|e| FeedbackReadError::Store(e.to_string()))?;
        match get(store, &node) {
            Ok(fb) => {
                if FEEDBACK_TERMINAL_STATES.contains(&fb.lifecycle_state.as_str()) {
                    continue;
                }
                if fb.rejection_reason.is_some() {
                    continue;
                }
                out.push(fb);
            }
            Err(FeedbackReadError::NotFound { .. }) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Return every feedback whose `dec:feedbackClass` literal equals `class`.
pub fn list_by_class(store: &Store, class: &str) -> Result<Vec<Feedback>, FeedbackReadError> {
    let iris = list_feedback_iris(store, None)?;
    let mut out: Vec<Feedback> = Vec::new();
    for iri in iris {
        let node = NamedNode::new(&iri).map_err(|e| FeedbackReadError::Store(e.to_string()))?;
        match get(store, &node) {
            Ok(fb) if fb.class == class => out.push(fb),
            Ok(_) => continue,
            Err(FeedbackReadError::NotFound { .. }) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Return every feedback whose `dec:targetRole` literal equals `role_id`.
pub fn list_by_target(store: &Store, role_id: &str) -> Result<Vec<Feedback>, FeedbackReadError> {
    let iris = list_feedback_iris(store, None)?;
    let mut out: Vec<Feedback> = Vec::new();
    for iri in iris {
        let node = NamedNode::new(&iri).map_err(|e| FeedbackReadError::Store(e.to_string()))?;
        match get(store, &node) {
            Ok(fb) if fb.target_role == role_id => out.push(fb),
            Ok(_) => continue,
            Err(FeedbackReadError::NotFound { .. }) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

fn list_feedback_iris(
    store: &Store,
    stream: Option<&NamedNode>,
) -> Result<Vec<String>, FeedbackReadError> {
    let q = build_list_query(stream);
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .map_err(|e| FeedbackReadError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| FeedbackReadError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get("f") {
            let iri = n.as_str().to_string();
            if !out.contains(&iri) {
                out.push(iri);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn build_list_query(stream: Option<&NamedNode>) -> String {
    let stream_clause = match stream {
        Some(s) => format!(
            "?f <{in_stream}> <{stream}> .",
            in_stream = IRI_DEC_IN_STREAM,
            stream = s.as_str()
        ),
        None => String::new(),
    };
    format!(
        "SELECT DISTINCT ?f WHERE {{ \
           {{ ?f a <{cls}> . {clause} }} \
           UNION \
           {{ GRAPH ?g {{ ?f a <{cls}> . {clause} }} }} \
         }}",
        cls = IRI_DEC_FEEDBACK,
        clause = stream_clause,
    )
}

fn collect_subject_quads(store: &Store, iri: &NamedNode) -> Result<Vec<Quad>, FeedbackReadError> {
    let mut out: Vec<Quad> = Vec::new();
    let s = iri.clone();
    for quad in store.quads_for_pattern(
        Some(oxigraph::model::Subject::NamedNode(s).as_ref()),
        None,
        None,
        None,
    ) {
        let q = quad.map_err(|e| FeedbackReadError::Store(e.to_string()))?;
        out.push(q);
    }
    Ok(out)
}

fn has_feedback_type(quads: &[Quad], iri: &NamedNode) -> bool {
    let rdf_type = super::artifact::RDF_TYPE;
    for q in quads {
        if q.predicate.as_str() != rdf_type {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_FEEDBACK {
            continue;
        }
        if let oxigraph::model::Subject::NamedNode(s) = &q.subject {
            if s == iri {
                return true;
            }
        }
    }
    false
}

fn parse_feedback(iri: &NamedNode, quads: &[Quad]) -> Result<Feedback, FeedbackReadError> {
    let required = parse_required_fields(iri, quads)?;
    let optional = parse_optional_fields(iri, quads)?;
    Ok(Feedback {
        iri: iri.clone(),
        class: required.class,
        severity: required.severity,
        target_role: required.target_role,
        evidence: required.evidence,
        recommendation: optional.recommendation,
        lifecycle_state: required.lifecycle_state,
        source_session: required.source_session,
        source_artifact: optional.source_artifact,
        addressing_artifact: optional.addressing_artifact,
        closed_by: optional.closed_by,
        rejection_reason: optional.rejection_reason,
        superseded_by: optional.superseded_by,
        routed_at: optional.routed_at,
        receiving_session: optional.receiving_session,
        disposition_override: optional.disposition_override,
        disposition_rationale: optional.disposition_rationale,
        in_stream: required.in_stream,
    })
}

struct RequiredFields {
    class: String,
    lifecycle_state: String,
    target_role: String,
    evidence: String,
    severity: Severity,
    source_session: NamedNode,
    in_stream: NamedNode,
}

fn parse_required_fields(
    iri: &NamedNode,
    quads: &[Quad],
) -> Result<RequiredFields, FeedbackReadError> {
    Ok(RequiredFields {
        class: take_one_literal(iri, quads, IRI_DEC_FEEDBACK_CLASS, "dec:feedbackClass")?,
        lifecycle_state: take_one_literal(
            iri,
            quads,
            IRI_DEC_LIFECYCLE_STATE,
            "dec:lifecycleState",
        )?,
        target_role: take_one_literal(iri, quads, IRI_DEC_TARGET_ROLE, "dec:targetRole")?,
        evidence: take_one_literal(iri, quads, IRI_DEC_EVIDENCE, "dec:evidence")?,
        severity: parse_severity(iri, quads)?,
        source_session: take_one_iri(iri, quads, IRI_DEC_SOURCE_SESSION, "dec:sourceSession")?,
        in_stream: take_one_iri(iri, quads, IRI_DEC_IN_STREAM, "dec:inStream")?,
    })
}

struct OptionalFields {
    recommendation: Option<String>,
    source_artifact: Option<NamedNode>,
    addressing_artifact: Option<NamedNode>,
    closed_by: Option<NamedNode>,
    rejection_reason: Option<String>,
    superseded_by: Option<NamedNode>,
    routed_at: Option<String>,
    receiving_session: Option<NamedNode>,
    disposition_override: Option<String>,
    disposition_rationale: Option<String>,
}

fn parse_optional_fields(
    iri: &NamedNode,
    quads: &[Quad],
) -> Result<OptionalFields, FeedbackReadError> {
    Ok(OptionalFields {
        recommendation: take_optional_literal(iri, quads, IRI_DEC_RECOMMENDATION)?,
        source_artifact: take_optional_iri(iri, quads, IRI_DEC_SOURCE_ARTIFACT)?,
        addressing_artifact: take_optional_iri(iri, quads, IRI_DEC_ADDRESSING_ARTIFACT)?,
        closed_by: take_optional_iri(iri, quads, IRI_DEC_CLOSED_BY)?,
        rejection_reason: take_optional_literal(iri, quads, IRI_DEC_REJECTION_REASON)?,
        superseded_by: take_optional_iri(iri, quads, IRI_DEC_SUPERSEDED_BY)?,
        routed_at: take_optional_literal(iri, quads, IRI_DEC_ROUTED_AT)?,
        receiving_session: take_optional_iri(iri, quads, IRI_DEC_RECEIVING_SESSION)?,
        disposition_override: take_optional_literal(iri, quads, IRI_DEC_DISPOSITION_OVERRIDE)?,
        disposition_rationale: take_optional_literal(iri, quads, IRI_DEC_DISPOSITION_RATIONALE)?,
    })
}

fn parse_severity(iri: &NamedNode, quads: &[Quad]) -> Result<Severity, FeedbackReadError> {
    let raw = take_optional_literal(iri, quads, IRI_DEC_SEVERITY)?;
    match raw {
        None => Ok(Severity::Warning),
        Some(value) => Severity::parse(&value).ok_or_else(|| FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("unknown dec:severity value {value:?}"),
        }),
    }
}

fn take_one_literal(
    iri: &NamedNode,
    quads: &[Quad],
    predicate: &str,
    label: &str,
) -> Result<String, FeedbackReadError> {
    let mut values = literal_values(quads, iri, predicate);
    match values.len() {
        0 => Err(FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => Ok(values.remove(0)),
        n => Err(FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

fn take_optional_literal(
    iri: &NamedNode,
    quads: &[Quad],
    predicate: &str,
) -> Result<Option<String>, FeedbackReadError> {
    let mut values = literal_values(quads, iri, predicate);
    match values.len() {
        0 => Ok(None),
        1 => Ok(Some(values.remove(0))),
        n => Err(FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

fn take_one_iri(
    iri: &NamedNode,
    quads: &[Quad],
    predicate: &str,
    label: &str,
) -> Result<NamedNode, FeedbackReadError> {
    let values = iri_values(quads, iri, predicate);
    match values.len() {
        0 => Err(FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => NamedNode::new(&values[0]).map_err(|e| FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("invalid IRI for {label}: {e}"),
        }),
        n => Err(FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

fn take_optional_iri(
    iri: &NamedNode,
    quads: &[Quad],
    predicate: &str,
) -> Result<Option<NamedNode>, FeedbackReadError> {
    let values = iri_values(quads, iri, predicate);
    match values.len() {
        0 => Ok(None),
        1 => Ok(Some(NamedNode::new(&values[0]).map_err(|e| {
            FeedbackReadError::Malformed {
                iri: iri.as_str().to_string(),
                detail: format!("invalid IRI for {predicate}: {e}"),
            }
        })?)),
        n => Err(FeedbackReadError::Malformed {
            iri: iri.as_str().to_string(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}
