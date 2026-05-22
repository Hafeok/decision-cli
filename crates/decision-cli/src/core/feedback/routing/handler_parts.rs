//! Private helpers split out of `handler.rs` to keep that file under
//! ADR-013 Rule 1's 400-line hard cap. Not part of the public surface.

use std::collections::BTreeSet;

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};

use crate::core::vocab::{
    orchestration_graph, routed_at as routed_at_pred, target_role as target_role_pred,
};

use super::handler::PendingFeedback;

pub(super) fn row_from_solution(
    sol: &oxigraph::sparql::QuerySolution,
    seen: &mut BTreeSet<String>,
) -> Option<PendingFeedback> {
    let Some(Term::NamedNode(feedback)) = sol.get("feedback").cloned() else {
        return None;
    };
    let Some(Term::Literal(class_lit)) = sol.get("class").cloned() else {
        return None;
    };
    let Some(Term::NamedNode(source_session)) = sol.get("sourceSession").cloned() else {
        return None;
    };
    if !seen.insert(feedback.as_str().to_string()) {
        return None;
    }
    let target_override = extract_target_override(sol);
    Some(PendingFeedback {
        feedback,
        class: class_lit.value().to_string(),
        target_override,
        source_session,
    })
}

fn extract_target_override(sol: &oxigraph::sparql::QuerySolution) -> Option<String> {
    match sol.get("targetOverride").cloned() {
        Some(Term::Literal(lit)) => Some(lit.value().to_string()),
        Some(Term::NamedNode(n)) => Some(n.as_str().to_string()),
        _ => None,
    }
}

pub(super) fn build_routed_evidence(
    feedback: &NamedNode,
    role: &str,
    now_rfc3339: &str,
    g: &GraphName,
) -> Vec<Quad> {
    vec![
        Quad::new(
            feedback.clone(),
            routed_at_pred().into_owned(),
            Literal::new_simple_literal(now_rfc3339),
            g.clone(),
        ),
        Quad::new(
            feedback.clone(),
            target_role_pred().into_owned(),
            Literal::new_simple_literal(role),
            g.clone(),
        ),
    ]
}

// Use the orchestration named graph as the canonical target for the
// transition; the prior `dec:targetRole` literal (if any) is removed
// by `apply` together with the lifecycle literal swap.
pub(super) fn canonical_graph_ref(g: &GraphName) -> NamedNodeRef<'_> {
    match g {
        GraphName::NamedNode(n) => n.as_ref(),
        _ => orchestration_graph(),
    }
}
