//! Struct → `Vec<Quad>` emission for `dec:Archetype` (FT-147).
//!
//! Symmetric with [`super::parser`] — every field the parser reads is
//! emitted here, witnessed by the round-trip tests.

use oxrdf::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::vocab::{
    IRI_DEC_APPLICATION_CONTRACT, IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT,
    IRI_DEC_APPLICATION_TASK_TYPE, IRI_DEC_ARCHETYPE, IRI_DEC_ARCHETYPE_AUDIT,
    IRI_DEC_ARCHETYPE_LAYER_ESTIMATE, IRI_DEC_ARCHETYPE_STATUS, IRI_DEC_ARCHETYPE_TITLE,
    IRI_DEC_COVERAGE_NOTE, IRI_DEC_INFRASTRUCTURE_CONTRACT_INSTANCE,
    IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE, IRI_DEC_INFRASTRUCTURE_TASK_TYPE,
    IRI_DEC_INSTANCE_VARIANCE, IRI_DEC_SEAM_AUDIT,
};

use super::types::Archetype;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD_FLOAT: &str = "http://www.w3.org/2001/XMLSchema#float";
const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

fn pred(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

impl Archetype {
    /// Emit the archetype as quads into `graph`.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let s = &self.id;

        let mut quads = vec![
            Quad::new(
                s.clone(),
                pred(RDF_TYPE),
                pred(IRI_DEC_ARCHETYPE),
                g.clone(),
            ),
            Quad::new(
                s.clone(),
                pred(IRI_DEC_ARCHETYPE_TITLE),
                Literal::new_simple_literal(&self.title),
                g.clone(),
            ),
            Quad::new(
                s.clone(),
                pred(IRI_DEC_ARCHETYPE_STATUS),
                Literal::new_simple_literal(self.status.as_str()),
                g.clone(),
            ),
            Quad::new(
                s.clone(),
                pred(IRI_DEC_APPLICATION_CONTRACT),
                self.application_contract.clone(),
                g.clone(),
            ),
            Quad::new(
                s.clone(),
                pred(IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE),
                self.infrastructure_contract_template.clone(),
                g.clone(),
            ),
        ];

        let iri_lists: [(&str, &Vec<NamedNode>); 5] = [
            (
                IRI_DEC_INFRASTRUCTURE_CONTRACT_INSTANCE,
                &self.infrastructure_contract_instances,
            ),
            (IRI_DEC_APPLICATION_TASK_TYPE, &self.application_task_types),
            (
                IRI_DEC_INFRASTRUCTURE_TASK_TYPE,
                &self.infrastructure_task_types,
            ),
            (IRI_DEC_ARCHETYPE_AUDIT, &self.archetype_audits),
            (IRI_DEC_SEAM_AUDIT, &self.seam_audits),
        ];
        for (p, targets) in iri_lists {
            for t in targets {
                quads.push(Quad::new(s.clone(), pred(p), t.clone(), g.clone()));
            }
        }

        quads.push(Quad::new(
            s.clone(),
            pred(IRI_DEC_ARCHETYPE_LAYER_ESTIMATE),
            Literal::new_typed_literal(
                self.evidence.archetype_layer_estimate.to_string(),
                pred(XSD_FLOAT),
            ),
            g.clone(),
        ));
        quads.push(Quad::new(
            s.clone(),
            pred(IRI_DEC_INSTANCE_VARIANCE),
            Literal::new_simple_literal(self.evidence.instance_variance.as_str()),
            g.clone(),
        ));
        quads.push(Quad::new(
            s.clone(),
            pred(IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT),
            Literal::new_typed_literal(
                self.evidence
                    .application_contract_held_invariant
                    .to_string(),
                pred(XSD_BOOLEAN),
            ),
            g.clone(),
        ));
        quads.push(Quad::new(
            s.clone(),
            pred(IRI_DEC_COVERAGE_NOTE),
            Literal::new_simple_literal(&self.evidence.coverage_note),
            g.clone(),
        ));

        quads.extend(self.provenance.to_quads(s, &g));
        quads
    }
}
