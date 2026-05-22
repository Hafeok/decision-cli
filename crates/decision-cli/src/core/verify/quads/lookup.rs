//! Env-IRI resolution helpers (per-step) for FT-037 StreamWriter integration.
//!
//! Split from `quads/mod.rs` to respect the ADR-013 file-length
//! budget. Also exposes subject-discovery helpers used by the
//! chokepoint.

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;

use crate::core::vocab::{
    IRI_DEC_ENVIRONMENT, IRI_DEC_STEPS, IRI_DEC_VERIFICATION_GRAPH, IRI_DEC_VERIFICATION_STEP,
};

use super::super::safety::{OpSource, SafetyError};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";

pub(super) fn step_subjects_in(quads: &[Quad]) -> Vec<NamedNode> {
    type_subjects(quads, IRI_DEC_VERIFICATION_STEP)
}

#[must_use]
pub fn touches_verification_artifacts(inserts: &[Quad]) -> bool {
    for q in inserts {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() == IRI_DEC_VERIFICATION_GRAPH || cls.as_str() == IRI_DEC_VERIFICATION_STEP {
            return true;
        }
    }
    false
}

fn type_subjects(quads: &[Quad], class_iri: &str) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != class_iri {
            continue;
        }
        if let Subject::NamedNode(s) = &q.subject {
            if !out.iter().any(|n| n == s) {
                out.push(s.clone());
            }
        }
    }
    out
}

/// Resolve the env IRI for `step`. Prefers the inserts (the common
/// graph-commit case where graph + steps + env-link are all in the
/// mutation), then falls back to the store (the step-append case).
pub(super) fn step_env_iri(
    inserts: &[Quad],
    store: &Store,
    step: &NamedNode,
) -> Result<NamedNode, SafetyError> {
    if let Some(env) = env_via_inserts(inserts, step) {
        return Ok(env);
    }
    if let Some(env) = env_via_store(store, step) {
        return Ok(env);
    }
    Err(SafetyError::UnknownOp {
        token: format!("<no env resolvable for step {step}>", step = step.as_str()),
        source: OpSource::Step,
    })
}

fn env_via_inserts(inserts: &[Quad], step: &NamedNode) -> Option<NamedNode> {
    for g in graphs_listing_step(inserts, step) {
        for q in inserts {
            if q.predicate.as_str() != IRI_DEC_ENVIRONMENT {
                continue;
            }
            if !matches_subject(q, &g) {
                continue;
            }
            if let Term::NamedNode(env) = &q.object {
                return Some(env.clone());
            }
        }
    }
    None
}

fn env_via_store(store: &Store, step: &NamedNode) -> Option<NamedNode> {
    let step_term: Term = step.clone().into();
    let rdf_first = NamedNode::new_unchecked(RDF_FIRST);
    for quad in store
        .quads_for_pattern(
            None,
            Some(rdf_first.as_ref()),
            Some(step_term.as_ref()),
            None,
        )
        .filter_map(Result::ok)
    {
        let list_head = quad.subject;
        let parent = walk_back_to_graph_in_store(store, &list_head)?;
        let env_pred = NamedNode::new_unchecked(IRI_DEC_ENVIRONMENT);
        for q in store
            .quads_for_pattern(
                Some(Subject::NamedNode(parent.clone()).as_ref()),
                Some(env_pred.as_ref()),
                None,
                None,
            )
            .filter_map(Result::ok)
        {
            if let Term::NamedNode(env) = q.object {
                return Some(env);
            }
        }
    }
    None
}

fn walk_back_to_graph_in_store(store: &Store, list_node: &Subject) -> Option<NamedNode> {
    let steps_pred = NamedNode::new_unchecked(IRI_DEC_STEPS);
    let rdf_rest = NamedNode::new_unchecked(RDF_REST);
    let mut current: Subject = list_node.clone();
    for _ in 0..1024 {
        let term = subject_as_term(&current)?;
        if let Some(graph) = graph_via_steps_predicate(store, &steps_pred, &term) {
            return Some(graph);
        }
        let Some(next) = step_back_one_rest_cell(store, &rdf_rest, &term) else {
            return None;
        };
        current = next;
    }
    None
}

fn subject_as_term(subject: &Subject) -> Option<Term> {
    match subject {
        Subject::NamedNode(n) => Some(n.clone().into()),
        Subject::BlankNode(b) => Some(b.clone().into()),
        Subject::Triple(_) => None,
    }
}

fn graph_via_steps_predicate(
    store: &Store,
    steps_pred: &NamedNode,
    term: &Term,
) -> Option<NamedNode> {
    for q in store
        .quads_for_pattern(None, Some(steps_pred.as_ref()), Some(term.as_ref()), None)
        .filter_map(Result::ok)
    {
        if let Subject::NamedNode(graph) = q.subject {
            return Some(graph);
        }
    }
    None
}

fn step_back_one_rest_cell(store: &Store, rdf_rest: &NamedNode, term: &Term) -> Option<Subject> {
    for q in store
        .quads_for_pattern(None, Some(rdf_rest.as_ref()), Some(term.as_ref()), None)
        .filter_map(Result::ok)
    {
        return Some(q.subject);
    }
    None
}

fn graphs_listing_step(inserts: &[Quad], step: &NamedNode) -> Vec<NamedNode> {
    let step_term: Term = step.clone().into();
    let mut list_cells_with_step: Vec<Subject> = Vec::new();
    for q in inserts {
        if q.predicate.as_str() != RDF_FIRST {
            continue;
        }
        if q.object == step_term {
            list_cells_with_step.push(q.subject.clone());
        }
    }
    let mut graphs: Vec<NamedNode> = Vec::new();
    for cell in list_cells_with_step {
        if let Some(graph) = walk_back_to_graph_in_inserts(inserts, cell) {
            if !graphs.iter().any(|g| g == &graph) {
                graphs.push(graph);
            }
        }
    }
    graphs
}

fn walk_back_to_graph_in_inserts(inserts: &[Quad], list_node: Subject) -> Option<NamedNode> {
    let mut current = list_node;
    for _ in 0..1024 {
        for q in inserts {
            if q.predicate.as_str() != IRI_DEC_STEPS {
                continue;
            }
            if matches_object(q, &current) {
                if let Subject::NamedNode(graph) = &q.subject {
                    return Some(graph.clone());
                }
            }
        }
        let mut moved = false;
        for q in inserts {
            if q.predicate.as_str() != RDF_REST {
                continue;
            }
            if matches_object(q, &current) {
                current = q.subject.clone();
                moved = true;
                break;
            }
        }
        if !moved {
            return None;
        }
    }
    None
}

fn matches_object(q: &Quad, s: &Subject) -> bool {
    match (s, &q.object) {
        (Subject::NamedNode(a), Term::NamedNode(b)) => a == b,
        (Subject::BlankNode(a), Term::BlankNode(b)) => a == b,
        _ => false,
    }
}

fn matches_subject(q: &Quad, s: &NamedNode) -> bool {
    matches!(&q.subject, Subject::NamedNode(n) if n == s)
}
