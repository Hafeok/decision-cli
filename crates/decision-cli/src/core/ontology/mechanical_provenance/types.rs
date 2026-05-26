//! Universal mechanical-provenance materialiser plus its value type (FT-069).
//!
//! The harness's session-completion handler hands a `SessionAttribution`
//! to GraphWriter, which calls [`materialise_quads`] to append the three
//! mechanical-provenance triples onto every artifact subject committed in
//! that session's mutation. Workers never construct this struct
//! themselves (ADR-008 + ADR-038).

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    generated_at_time_pred, was_attributed_to_pred, was_generated_by_pred, IRI_XSD_DATE_TIME,
};

/// Harness-supplied snapshot of the producing Session and its attributed
/// Agents. Single-writer per graph by construction (GraphWriter is the
/// only mutation chokepoint per FT-001 / ADR-041), which is why
/// `prov:generatedAtTime` is monotonically increasing inside a named
/// graph (FT-069 §Invariants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAttribution {
    /// The Session that produced the artifact (single-valued per FT-069).
    pub session: NamedNode,
    /// One or more Agents the Session was attributed to. Multi-valued
    /// allowed when a session is jointly attributed (e.g. a
    /// human-supervised LLM session). Empty is rejected by SHACL.
    pub attributed_to: Vec<NamedNode>,
    /// RFC-3339 / xsd:dateTime literal of the GraphWriter commit. The
    /// caller passes the exact string the transaction clock produced so
    /// validation can be lexical.
    pub generated_at_time: String,
}

impl SessionAttribution {
    /// Construct an attribution snapshot. Caller is responsible for
    /// supplying a well-formed `xsd:dateTime` literal; SHACL will
    /// reject malformed strings at commit time.
    #[must_use]
    pub fn new(
        session: NamedNode,
        attributed_to: Vec<NamedNode>,
        generated_at_time: impl Into<String>,
    ) -> Self {
        Self {
            session,
            attributed_to,
            generated_at_time: generated_at_time.into(),
        }
    }
}

/// Append the universal mechanical-provenance triples to a quad bundle
/// for `artifact`. Idempotent for the *predicates* — caller is
/// responsible for not double-materialising on the same subject (the
/// GraphWriter chokepoint enforces single materialisation per commit
/// per ADR-041).
#[must_use]
pub fn materialise_quads(
    artifact: &NamedNode,
    attribution: &SessionAttribution,
    graph: NamedNodeRef<'_>,
) -> Vec<Quad> {
    let g: GraphName = graph.into_owned().into();
    let mut quads = Vec::with_capacity(2 + attribution.attributed_to.len());
    quads.push(named_quad(
        artifact,
        was_generated_by_pred(),
        &attribution.session,
        &g,
    ));
    for agent in &attribution.attributed_to {
        quads.push(named_quad(artifact, was_attributed_to_pred(), agent, &g));
    }
    quads.push(typed_literal_quad(
        artifact,
        generated_at_time_pred(),
        &attribution.generated_at_time,
        IRI_XSD_DATE_TIME,
        &g,
    ));
    quads
}

fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}

fn typed_literal_quad(
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
