//! Reusable dual-provenance value for typed artifacts (FT-072 / FT-073 /
//! ADR-038).
//!
//! Every graph-resident artifact carries a mechanical PROV-O block
//! (`wasGeneratedBy`, `wasAttributedTo`, `generatedAtTime`) plus zero or
//! more motivational edges drawn from the FT-070 predicate vocabulary.
//! Earlier artifact types materialise the mechanical block at the write
//! chokepoint; the archetype layer (FT-147+) carries provenance as a
//! struct field so parsers and emitters round-trip it symmetrically.

use oxrdf::{GraphName, Literal, NamedNode, Quad, Term};

use crate::vocab::{
    IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL, IRI_PROV_WAS_GENERATED_BY,
    IRI_XSD_DATE_TIME,
};

/// Mechanical + motivational provenance carried by an artifact subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// PROV-O `wasGeneratedBy` — the producing session/activity IRI.
    pub was_generated_by: NamedNode,
    /// PROV-O `wasAttributedTo` — the responsible agent IRI.
    pub was_attributed_to: NamedNode,
    /// PROV-O `generatedAtTime` — xsd:dateTime lexical form.
    pub generated_at_time: String,
    /// Motivational edges (FT-070): predicate → upstream artifact.
    pub motivational: Vec<MotivationalEdge>,
}

/// One motivational edge, e.g. `dec:respondsTo <…/FT-147>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotivationalEdge {
    /// Motivational predicate IRI (must be one of the FT-070 vocabulary).
    pub predicate: NamedNode,
    /// Upstream artifact the edge points at.
    pub target: NamedNode,
}

impl Provenance {
    /// Emit the provenance quads for `subject` into `graph`.
    #[must_use]
    pub fn to_quads(&self, subject: &NamedNode, graph: &GraphName) -> Vec<Quad> {
        let mut quads = vec![
            Quad::new(
                subject.clone(),
                NamedNode::new_unchecked(IRI_PROV_WAS_GENERATED_BY),
                self.was_generated_by.clone(),
                graph.clone(),
            ),
            Quad::new(
                subject.clone(),
                NamedNode::new_unchecked(IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL),
                self.was_attributed_to.clone(),
                graph.clone(),
            ),
            Quad::new(
                subject.clone(),
                NamedNode::new_unchecked(IRI_PROV_GENERATED_AT_TIME),
                Literal::new_typed_literal(
                    &self.generated_at_time,
                    NamedNode::new_unchecked(IRI_XSD_DATE_TIME),
                ),
                graph.clone(),
            ),
        ];
        for edge in &self.motivational {
            quads.push(Quad::new(
                subject.clone(),
                edge.predicate.clone(),
                edge.target.clone(),
                graph.clone(),
            ));
        }
        quads
    }

    /// Reassemble the provenance block for `subject` from `quads`.
    ///
    /// Returns `None` when the mechanical block is incomplete — callers
    /// decide whether that is an error (parsers under FT-073 treat it as
    /// one) or an artifact grandfathered per ADR-042.
    #[must_use]
    pub fn from_quads(
        quads: &[Quad],
        subject: &NamedNode,
        motivational_predicates: &[&str],
    ) -> Option<Self> {
        let mut was_generated_by = None;
        let mut was_attributed_to = None;
        let mut generated_at_time = None;
        let mut motivational = Vec::new();

        for q in quads
            .iter()
            .filter(|q| q.subject == (*subject).clone().into())
        {
            let pred = q.predicate.as_str();
            if pred == IRI_PROV_WAS_GENERATED_BY {
                if let Term::NamedNode(n) = &q.object {
                    was_generated_by = Some(n.clone());
                }
            } else if pred == IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL {
                if let Term::NamedNode(n) = &q.object {
                    was_attributed_to = Some(n.clone());
                }
            } else if pred == IRI_PROV_GENERATED_AT_TIME {
                if let Term::Literal(l) = &q.object {
                    generated_at_time = Some(l.value().to_string());
                }
            } else if motivational_predicates.contains(&pred) {
                if let Term::NamedNode(n) = &q.object {
                    motivational.push(MotivationalEdge {
                        predicate: q.predicate.clone(),
                        target: n.clone(),
                    });
                }
            }
        }

        Some(Self {
            was_generated_by: was_generated_by?,
            was_attributed_to: was_attributed_to?,
            generated_at_time: generated_at_time?,
            motivational,
        })
    }
}
