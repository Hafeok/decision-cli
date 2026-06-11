//! In-memory `dec:SessionRecord` shape + RDF serialisation (FT-057).

use oxrdf::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::ontology::role_binding::TriggerSignal;
use crate::vocab::{
    escalated_from_pred, escalated_to_pred, escalation_reason_pred, input_tokens_base_pred,
    input_tokens_cache_hit_pred, input_tokens_cache_write_pred, output_tokens_pred,
    session_capability_pred,
};

/// IRI of `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
/// IRI of the `xsd:integer` datatype.
pub const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";
/// IRI of the `dec:Session` class.
pub const IRI_DEC_SESSION: &str = "https://decision-cli.dev/ns#Session";

/// In-memory record of a `dec:Session` (a.k.a. `dec:SessionRecord`) — the
/// new fields landed by FT-057. Existing PROV-O / feature-id / status
/// fields written by the dispatcher live on other modules; this struct
/// is intentionally scoped to FT-057's additions so callers can extend
/// without rewriting the unrelated dispatcher write paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    /// Stable session IRI.
    pub iri: NamedNode,
    /// Optional escalated-from edge (FT-057 / ADR-034). Set on the
    /// escalated session; the dispatcher mirrors the inverse edge on
    /// the prior session via [`SessionRecord::escalated_to_quad`].
    pub escalated_from: Option<NamedNode>,
    /// Optional escalation reason (FT-057 / ADR-034). Required iff
    /// `escalated_from` is set; both absent on root sessions.
    pub escalation_reason: Option<TriggerSignal>,
    /// Input tokens billed at the capability's `cost_input_per_m`.
    pub input_tokens_base: u64,
    /// Cache-write input tokens (Anthropic only; 0 on Scaleway).
    pub input_tokens_cache_write: u64,
    /// Cache-hit input tokens (Anthropic only; 0 on Scaleway).
    pub input_tokens_cache_hit: u64,
    /// Output tokens emitted by the session.
    pub output_tokens: u64,
    /// Capability that resolved for this session (FT-057 — records the
    /// full reproducibility tuple alongside the bundle hash).
    pub capability: NamedNode,
}

/// Borrowed view used by the dispatcher when composing the escalation
/// triple on the *prior* session in the same mutation that writes a
/// new escalated session.
#[derive(Debug, Clone)]
pub struct SessionRecordRef<'a> {
    /// IRI of the session whose `dec:escalated_to` is being set.
    pub prior: &'a NamedNode,
    /// IRI of the newly-dispatched escalated session.
    pub successor: &'a NamedNode,
}

impl SessionRecord {
    /// Emit the FT-057 quads — class, escalation edges (if any), and the
    /// four token-count + capability triples. Existing dispatcher write
    /// paths handle the unrelated session fields (feature id, PROV-O
    /// lineage, etc.) and concatenate the slices.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let mut quads = self.header_quads(&g);
        quads.extend(self.escalation_quads(&g));
        quads.extend(self.token_quads(&g));
        quads
    }

    /// Emit the bidirectional `dec:escalated_to` triple on `prior`,
    /// pointing at `self.iri`. Returns `None` when `prior` is `None`.
    #[must_use]
    pub fn escalated_to_quad(&self, graph: NamedNodeRef<'_>) -> Option<Quad> {
        let prior = self.escalated_from.as_ref()?;
        let g: GraphName = graph.into_owned().into();
        Some(named_quad(prior, escalated_to_pred(), &self.iri, &g))
    }

    fn header_quads(&self, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        vec![
            Quad::new(
                self.iri.clone(),
                rdf_type,
                NamedNode::new_unchecked(IRI_DEC_SESSION),
                g.clone(),
            ),
            named_quad(&self.iri, session_capability_pred(), &self.capability, g),
        ]
    }

    fn escalation_quads(&self, g: &GraphName) -> Vec<Quad> {
        let mut out = Vec::new();
        if let Some(prior) = &self.escalated_from {
            out.push(named_quad(&self.iri, escalated_from_pred(), prior, g));
        }
        if let Some(reason) = &self.escalation_reason {
            out.push(literal_quad(
                &self.iri,
                escalation_reason_pred(),
                reason.as_str(),
                g,
            ));
        }
        out
    }

    fn token_quads(&self, g: &GraphName) -> Vec<Quad> {
        vec![
            typed_literal_quad(
                &self.iri,
                input_tokens_base_pred(),
                &self.input_tokens_base.to_string(),
                XSD_INTEGER,
                g,
            ),
            typed_literal_quad(
                &self.iri,
                input_tokens_cache_write_pred(),
                &self.input_tokens_cache_write.to_string(),
                XSD_INTEGER,
                g,
            ),
            typed_literal_quad(
                &self.iri,
                input_tokens_cache_hit_pred(),
                &self.input_tokens_cache_hit.to_string(),
                XSD_INTEGER,
                g,
            ),
            typed_literal_quad(
                &self.iri,
                output_tokens_pred(),
                &self.output_tokens.to_string(),
                XSD_INTEGER,
                g,
            ),
        ]
    }
}

/// Build a plain-literal quad `(s, p, "value", g)`.
pub fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

/// Build a typed-literal quad `(s, p, "value"^^datatype, g)`.
pub fn typed_literal_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    value: &str,
    datatype: &str,
    g: &GraphName,
) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype)),
        g.clone(),
    )
}

/// Build an IRI-object quad `(s, p, o, g)`.
pub fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}
