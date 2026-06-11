//! In-memory `Feedback` shape + RDF serialisation (ADR-022).

use oxrdf::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::vocab::{
    addressing_artifact, closed_by, disposition_override, disposition_rationale, evidence,
    feedback_class, feedback_class_pred, in_stream, lifecycle_state, receiving_session,
    recommendation, rejection_reason, routed_at, severity, source_artifact, source_session,
    superseded_by, target_role,
};

/// IRI of `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Severity hint carried on every emitted feedback.
///
/// Not validated by `sh:in` at the FT-026 level — the SHACL shape only
/// constrains cardinality. Workers and the SDK should default to
/// `Severity::Warning`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Informational signal.
    Info,
    /// Warning — the default for most class-vocabulary entries.
    Warning,
    /// Error — the action could not proceed without addressing.
    Error,
}

impl Severity {
    /// Stable wire string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Parse from the wire string. Unknown values fall back to `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// In-memory feedback artifact mirroring the FT-026 outputs.
///
/// Class controlled vocabulary (FT-028) and lifecycle state machine
/// (FT-027) layer on top of this shape; this struct represents the
/// schema substrate every later slice consumes.
#[derive(Debug, Clone)]
pub struct Feedback {
    /// Stable IRI for the feedback artifact.
    pub iri: NamedNode,
    /// Controlled-vocabulary class tag (ADR-023). Validated as `sh:in`
    /// in FT-028; here we only ensure cardinality.
    pub class: String,
    /// Severity hint.
    pub severity: Severity,
    /// Role id (matching `dec:roleId`) the feedback is routed to.
    pub target_role: String,
    /// Non-empty free-form citation.
    pub evidence: String,
    /// Optional suggested fix.
    pub recommendation: Option<String>,
    /// Current lifecycle state (ADR-024). FT-027 owns the state machine;
    /// FT-026 stores the literal as-is and enforces cardinality only.
    pub lifecycle_state: String,
    /// PROV-O linked Session that emitted the feedback.
    pub source_session: NamedNode,
    /// Optional bundled artifact the feedback is about.
    pub source_artifact: Option<NamedNode>,
    /// Set on lifecycle transition to `addressed` — the resolving artifact.
    pub addressing_artifact: Option<NamedNode>,
    /// Actor (session or human) that closed the loop.
    pub closed_by: Option<NamedNode>,
    /// Free-form rationale recorded on rejection.
    pub rejection_reason: Option<String>,
    /// Newer feedback artifact that subsumes this one.
    pub superseded_by: Option<NamedNode>,
    /// RFC3339 timestamp the orchestrator transitioned this feedback to routed.
    pub routed_at: Option<String>,
    /// Session that received the routed feedback.
    pub receiving_session: Option<NamedNode>,
    /// Per-emission blocking / non-blocking override (ADR-025).
    pub disposition_override: Option<String>,
    /// Operator-facing rationale for the override.
    pub disposition_rationale: Option<String>,
    /// Active stream IRI (`dec:inStream`, ADR-005).
    pub in_stream: NamedNode,
}

impl Feedback {
    /// Serialise the feedback to RDF quads in the supplied named graph.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let mut quads = self.required_quads(&g);
        quads.extend(self.optional_quads(&g));
        quads
    }

    fn required_quads(&self, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let cls = feedback_class();
        vec![
            Quad::new(self.iri.clone(), rdf_type, cls, g.clone()),
            literal_quad(&self.iri, feedback_class_pred(), &self.class, g),
            literal_quad(&self.iri, lifecycle_state(), &self.lifecycle_state, g),
            literal_quad(&self.iri, target_role(), &self.target_role, g),
            literal_quad(&self.iri, evidence(), &self.evidence, g),
            literal_quad(&self.iri, severity(), self.severity.as_str(), g),
            named_quad(&self.iri, source_session(), &self.source_session, g),
            named_quad(&self.iri, in_stream(), &self.in_stream, g),
        ]
    }

    fn optional_quads(&self, g: &GraphName) -> Vec<Quad> {
        let mut out: Vec<Quad> = Vec::new();
        self.push_optional_literals(&mut out, g);
        self.push_optional_named(&mut out, g);
        out
    }

    fn push_optional_literals(&self, out: &mut Vec<Quad>, g: &GraphName) {
        if let Some(rec) = &self.recommendation {
            out.push(literal_quad(&self.iri, recommendation(), rec, g));
        }
        if let Some(reason) = &self.rejection_reason {
            out.push(literal_quad(&self.iri, rejection_reason(), reason, g));
        }
        if let Some(ts) = &self.routed_at {
            out.push(literal_quad(&self.iri, routed_at(), ts, g));
        }
        if let Some(disp) = &self.disposition_override {
            out.push(literal_quad(&self.iri, disposition_override(), disp, g));
        }
        if let Some(reason) = &self.disposition_rationale {
            out.push(literal_quad(&self.iri, disposition_rationale(), reason, g));
        }
    }

    fn push_optional_named(&self, out: &mut Vec<Quad>, g: &GraphName) {
        if let Some(art) = &self.source_artifact {
            out.push(named_quad(&self.iri, source_artifact(), art, g));
        }
        if let Some(art) = &self.addressing_artifact {
            out.push(named_quad(&self.iri, addressing_artifact(), art, g));
        }
        if let Some(actor) = &self.closed_by {
            out.push(named_quad(&self.iri, closed_by(), actor, g));
        }
        if let Some(sup) = &self.superseded_by {
            out.push(named_quad(&self.iri, superseded_by(), sup, g));
        }
        if let Some(sess) = &self.receiving_session {
            out.push(named_quad(&self.iri, receiving_session(), sess, g));
        }
    }
}

pub(super) fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

pub(super) fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}
