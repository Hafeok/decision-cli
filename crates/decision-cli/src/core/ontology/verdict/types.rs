//! In-memory `VerificationVerdict` shape + RDF serialisation (ADR-018).

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    amendment_guidance, in_stream, rationale, verdict, verification_verdict_class, violates,
    VERDICT_AMENDMENT_REQUIRED, VERDICT_APPROVED, VERDICT_REJECTED,
};

pub(super) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub(super) const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
pub(super) const PROV_USED: &str = "http://www.w3.org/ns/prov#used";

/// One of `approved`, `rejected`, `amendment-required` (ADR-018 §SHACL shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    /// The action artifact satisfies its feature_spec and TCs.
    Approved,
    /// The action artifact does not satisfy the spec; no corrective amendment is possible.
    Rejected,
    /// The action artifact is on the right track but needs specific amendments.
    AmendmentRequired,
}

impl Verdict {
    /// Stable wire string (`approved` / `rejected` / `amendment-required`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Approved => VERDICT_APPROVED,
            Self::Rejected => VERDICT_REJECTED,
            Self::AmendmentRequired => VERDICT_AMENDMENT_REQUIRED,
        }
    }

    /// Parse a verdict from its wire string. Returns `None` for unknown
    /// inputs (the SHACL `sh:in` constraint rejects them at commit time).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            VERDICT_APPROVED => Some(Self::Approved),
            VERDICT_REJECTED => Some(Self::Rejected),
            VERDICT_AMENDMENT_REQUIRED => Some(Self::AmendmentRequired),
            _ => None,
        }
    }
}

/// In-memory verdict artifact.
#[derive(Debug, Clone)]
pub struct VerdictArtifact {
    /// Stable IRI of the verdict artifact.
    pub iri: NamedNode,
    /// The three-value verdict.
    pub verdict: Verdict,
    /// Free-form prose, ≥ 20 chars (SHACL `sh:minLength`).
    pub rationale: String,
    /// References to violated TCs / ADRs. Required when `verdict ≠ Approved`.
    pub violates: Vec<NamedNode>,
    /// Required when `verdict = AmendmentRequired`; otherwise must be `None`.
    pub amendment_guidance: Option<String>,
    /// The `InterpretationSession` that produced the verdict (ADR-019).
    pub generated_by: NamedNode,
    /// The action artifacts the verdict refers to.
    pub used: Vec<NamedNode>,
    /// Active stream IRI (`dec:inStream`, ADR-005).
    pub in_stream: NamedNode,
}

impl VerdictArtifact {
    /// Serialise the verdict to RDF quads in the supplied named graph.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let mut quads = self.header_quads(&g);
        quads.extend(self.provenance_quads(&g));
        quads.extend(self.violates_quads(&g));
        if let Some(guidance) = &self.amendment_guidance {
            quads.push(literal_quad(&self.iri, amendment_guidance(), guidance, &g));
        }
        quads
    }

    fn header_quads(&self, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let cls = verification_verdict_class();
        vec![
            Quad::new(self.iri.clone(), rdf_type, cls, g.clone()),
            literal_quad(&self.iri, verdict(), self.verdict.as_str(), g),
            literal_quad(&self.iri, rationale(), &self.rationale, g),
            named_quad(&self.iri, in_stream(), &self.in_stream, g),
        ]
    }

    fn provenance_quads(&self, g: &GraphName) -> Vec<Quad> {
        let gen_by = NamedNodeRef::new_unchecked(PROV_WAS_GENERATED_BY);
        let used = NamedNodeRef::new_unchecked(PROV_USED);
        let mut quads = vec![named_quad(&self.iri, gen_by, &self.generated_by, g)];
        for u in &self.used {
            quads.push(named_quad(&self.iri, used, u, g));
        }
        quads
    }

    fn violates_quads(&self, g: &GraphName) -> Vec<Quad> {
        let pred = violates();
        self.violates
            .iter()
            .map(|v| named_quad(&self.iri, pred, v, g))
            .collect()
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
