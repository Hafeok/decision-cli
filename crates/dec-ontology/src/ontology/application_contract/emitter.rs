//! Struct → `Vec<Quad>` emission for `dec:ApplicationContract` (FT-148).
//!
//! Symmetric with [`super::parser`]; Conventions emit inline as
//! sub-resources, witnessed by the round-trip tests.

use oxrdf::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::vocab::{
    IRI_DEC_APPLICATION_CONTRACT_CLASS, IRI_DEC_CONTRACT_ARCHETYPE, IRI_DEC_CONVENTION_AUDIT_ID,
    IRI_DEC_CONVENTION_BODY_PATH, IRI_DEC_CONVENTION_CHECKABLE, IRI_DEC_CONVENTION_CLASS,
    IRI_DEC_CONVENTION_NAME, IRI_DEC_CROSS_CUTTING, IRI_DEC_ENDPOINT_CONVENTION,
    IRI_DEC_FEATURE_ORGANISATION, IRI_DEC_LANGUAGE_RUNTIME, IRI_DEC_LAYERING_RULE,
    IRI_DEC_PERSISTENCE_MODEL,
};

use super::types::{ApplicationContract, Convention};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

fn pred(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

fn convention_quads(
    owner_pred: &str,
    c: &Convention,
    subject: &NamedNode,
    g: &GraphName,
) -> Vec<Quad> {
    let mut quads = vec![
        Quad::new(subject.clone(), pred(owner_pred), c.id.clone(), g.clone()),
        Quad::new(
            c.id.clone(),
            pred(RDF_TYPE),
            pred(IRI_DEC_CONVENTION_CLASS),
            g.clone(),
        ),
        Quad::new(
            c.id.clone(),
            pred(IRI_DEC_CONVENTION_NAME),
            Literal::new_simple_literal(&c.name),
            g.clone(),
        ),
        Quad::new(
            c.id.clone(),
            pred(IRI_DEC_CONVENTION_BODY_PATH),
            Literal::new_simple_literal(c.body_path.to_string_lossy()),
            g.clone(),
        ),
        Quad::new(
            c.id.clone(),
            pred(IRI_DEC_CONVENTION_CHECKABLE),
            Literal::new_typed_literal(c.checkable.to_string(), pred(XSD_BOOLEAN)),
            g.clone(),
        ),
    ];
    if let Some(audit) = &c.audit_id {
        quads.push(Quad::new(
            c.id.clone(),
            pred(IRI_DEC_CONVENTION_AUDIT_ID),
            audit.clone(),
            g.clone(),
        ));
    }
    quads
}

impl ApplicationContract {
    /// Emit the contract and its inline Conventions as quads into `graph`.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let s = &self.id;
        let mut quads = vec![
            Quad::new(
                s.clone(),
                pred(RDF_TYPE),
                pred(IRI_DEC_APPLICATION_CONTRACT_CLASS),
                g.clone(),
            ),
            Quad::new(
                s.clone(),
                pred(IRI_DEC_CONTRACT_ARCHETYPE),
                self.archetype.clone(),
                g.clone(),
            ),
        ];
        for (owner_pred, convention) in [
            (IRI_DEC_LANGUAGE_RUNTIME, &self.language_runtime),
            (IRI_DEC_LAYERING_RULE, &self.layering_rule),
            (IRI_DEC_FEATURE_ORGANISATION, &self.feature_organisation),
            (IRI_DEC_PERSISTENCE_MODEL, &self.persistence_model),
            (IRI_DEC_ENDPOINT_CONVENTION, &self.endpoint_convention),
        ] {
            quads.extend(convention_quads(owner_pred, convention, s, &g));
        }
        for convention in &self.cross_cutting {
            quads.extend(convention_quads(IRI_DEC_CROSS_CUTTING, convention, s, &g));
        }
        quads.extend(self.provenance.to_quads(s, &g));
        quads
    }
}
