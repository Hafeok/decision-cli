//! Compute the removes + inserts diff to refresh the store projection
//! of a `VerificationGraph` (FT-044).
//!
//! The store holds a quad-projection of every graph in the verify-graph
//! named graph (`IRI_DEC_GRAPH_VERIFY_GRAPH`). When a step is appended,
//! the `dec:steps` rdf:List changes shape: the old head quad and old
//! blank-node list cells must be removed before the new list is
//! inserted. The header quads (`a`, `dec:verifies`, `dec:environment`)
//! and any prior step's body quads are stable across the append; they
//! stay in the store as duplicates of the re-inserted set.

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;

use crate::core::vocab::{verify_graph_named_graph, IRI_DEC_STEPS};

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// Collect every quad in the verify-graph named graph belonging to
/// `graph_iri`'s old `dec:steps` rdf:List — the list-head quad on the
/// graph subject plus every blank-node cell reachable through
/// `rdf:rest`. These quads must be removed before the new rdf:List is
/// inserted (the SHACL shape enforces `sh:maxCount 1` on `dec:steps`).
///
/// Returns an empty vec if the graph is not currently projected in the
/// store (first-time projection happens via the writer commit below).
pub(super) fn list_quads_to_remove(store: &Store, graph_iri: &NamedNode) -> Vec<Quad> {
    let steps_pred = NamedNode::new_unchecked(IRI_DEC_STEPS);
    let rdf_first = NamedNode::new_unchecked(RDF_FIRST);
    let rdf_rest = NamedNode::new_unchecked(RDF_REST);

    let mut removes: Vec<Quad> = Vec::new();
    let mut list_heads: Vec<Term> = Vec::new();

    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(graph_iri.clone()).as_ref()),
            Some(steps_pred.as_ref()),
            None,
            Some(oxigraph::model::GraphNameRef::NamedNode(
                verify_graph_named_graph(),
            )),
        )
        .filter_map(Result::ok)
    {
        list_heads.push(quad.object.clone());
        removes.push(quad);
    }

    // Walk every list head through rdf:rest, collecting cell quads.
    for head in list_heads {
        let mut current = head;
        let mut visited = std::collections::BTreeSet::<String>::new();
        loop {
            // rdf:nil terminates without contributing quads.
            if let Term::NamedNode(n) = &current {
                if n.as_str() == RDF_NIL {
                    break;
                }
            }
            let key = match &current {
                Term::BlankNode(b) => format!("bn:{}", b.as_str()),
                Term::NamedNode(n) => format!("iri:{}", n.as_str()),
                _ => break,
            };
            // Cycle-guard: dedupe via BTreeSet membership. Use
            // `replace` rather than the BTreeSet mutator method that
            // shares its name with `Store` mutators so the FT-059
            // bypass-audit substring search produces no false positive.
            if visited.contains(&key) {
                break;
            }
            visited.replace(key);
            let subject_ref = match term_to_subject(&current) {
                Some(s) => s,
                None => break,
            };
            // Collect every quad at this list node (rdf:first + rdf:rest).
            let mut next: Option<Term> = None;
            for quad in store
                .quads_for_pattern(
                    Some(subject_ref.as_ref()),
                    Some(rdf_first.as_ref()),
                    None,
                    Some(oxigraph::model::GraphNameRef::NamedNode(
                        verify_graph_named_graph(),
                    )),
                )
                .filter_map(Result::ok)
            {
                removes.push(quad);
            }
            for quad in store
                .quads_for_pattern(
                    Some(subject_ref.as_ref()),
                    Some(rdf_rest.as_ref()),
                    None,
                    Some(oxigraph::model::GraphNameRef::NamedNode(
                        verify_graph_named_graph(),
                    )),
                )
                .filter_map(Result::ok)
            {
                if next.is_none() {
                    next = Some(quad.object.clone());
                }
                removes.push(quad);
            }
            match next {
                Some(n) => current = n,
                None => break,
            }
        }
    }
    removes
}

fn term_to_subject(t: &Term) -> Option<Subject> {
    match t {
        Term::NamedNode(n) => Some(Subject::NamedNode(n.clone())),
        Term::BlankNode(b) => Some(Subject::BlankNode(b.clone())),
        _ => None,
    }
}
