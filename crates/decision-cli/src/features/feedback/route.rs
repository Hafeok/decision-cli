//! `dec feedback route <iri> --to <role-id>` (FT-033).
//!
//! Records a manual `dec:routingOverride` on a `produced` feedback
//! artifact so the next FT-029 routing pass picks the operator's
//! choice instead of the routing-table default. The override is also
//! tagged with `dec:routingOverrideActor` carrying the operator's
//! identity (resolved from environment / git config by the CLI
//! adapter — this module accepts the identity as a string).
//!
//! Phase A scope (per FT-033 §Outputs): the override only applies
//! pre-routing. If the feedback has already left `produced`, the
//! command refuses — re-routing rejected feedback requires a new
//! emission and is a Phase B path.

use std::path::Path;

use anyhow::{anyhow, Result};
use oxi_events::Mutation;
use oxigraph::model::{Literal, NamedNode, Quad, Subject};
use oxigraph::store::Store;

use crate::core::feedback::class::FeedbackClass;
use crate::core::feedback::routing::{override_permitted_for, OverrideActor};
use crate::core::feedback::transition::read_prior_state;
use crate::core::feedback::{get, LifecycleState};
use crate::core::role_catalog::role::lookup as lookup_role;
use crate::core::vocab::{
    orchestration_graph, routing_override, routing_override_actor, IRI_DEC_ROUTING_OVERRIDE,
};

use super::format::{json_escape, json_opt};
use super::store_io::WritableStore;

/// Successful outcome of a route override.
#[derive(Debug, Clone)]
pub struct RouteOutcome {
    /// IRI of the routed feedback.
    pub feedback_iri: String,
    /// New override target role recorded on the artifact.
    pub target_role: String,
    /// Identity recorded via `dec:routingOverrideActor`.
    pub actor: String,
}

/// Errors from `dec feedback route`.
#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    /// Feedback IRI doesn't parse.
    #[error("invalid feedback IRI: {0}")]
    InvalidIri(String),
    /// No feedback artifact at the given IRI.
    #[error("feedback not found: <{0}>")]
    NotFound(String),
    /// Feedback's class literal isn't one of the six ADR-023 values.
    #[error("feedback <{feedback}> has unknown class {class:?}")]
    UnknownClass {
        /// Feedback IRI.
        feedback: String,
        /// Class literal observed.
        class: String,
    },
    /// Override target role is not in the routing-table allowlist for
    /// this class and actor combination.
    #[error(
        "override not permitted for class `{class}` (actor={actor:?}); \
         the human operator may not override this class's routing"
    )]
    OverrideNotPermitted {
        /// Class literal.
        class: String,
        /// Override actor (Phase A: always `Human` from CLI).
        actor: OverrideActor,
    },
    /// Override target role does not exist in the role catalog.
    #[error("unknown target role: `{0}` — not in dec:Role catalog")]
    UnknownTargetRole(String),
    /// Feedback is not in `produced` state — Phase A only overrides
    /// pre-routing.
    #[error(
        "cannot route feedback <{feedback}>: state is {state}, expected `produced` \
         (Phase A overrides only apply pre-routing — FT-033 §Outputs)"
    )]
    WrongState {
        /// Feedback IRI.
        feedback: String,
        /// State observed.
        state: String,
    },
    /// Wrapped store / writer failure.
    #[error("{0}")]
    Other(String),
}

/// Resolve the operator identity from the environment.
///
/// Search order: `DEC_USER`, `USER`, `LOGNAME`, then the literal
/// `"anonymous"`. Slice 3 deliberately keeps the resolver dependency-
/// free; Phase B can layer `git config user.email` here without
/// breaking the contract.
#[must_use]
pub fn resolve_actor() -> String {
    for key in ["DEC_USER", "USER", "LOGNAME"] {
        if let Ok(v) = std::env::var(key) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "anonymous".to_string()
}

/// Apply a `dec feedback route --to <role-id>` override.
pub fn route(
    workdir: &Path,
    feedback_iri: &str,
    target_role: &str,
    actor: &str,
) -> Result<RouteOutcome, RouteError> {
    let ws = WritableStore::open(workdir).map_err(|e| RouteError::Other(format!("{e:#}")))?;
    let fb_node = NamedNode::new(feedback_iri)
        .map_err(|_| RouteError::InvalidIri(feedback_iri.to_string()))?;
    let fb = load_feedback(&ws, &fb_node)?;
    ensure_produced_state(&ws, &fb_node, feedback_iri)?;
    let class = parse_class(&fb, feedback_iri)?;
    ensure_override_permitted(class, &fb.class)?;
    ensure_target_role_exists(&ws, target_role)?;
    apply_override(&ws, &fb_node, target_role, actor)?;
    ws.persist()
        .map_err(|e| RouteError::Other(format!("persisting store: {e:#}")))?;
    Ok(RouteOutcome {
        feedback_iri: feedback_iri.to_string(),
        target_role: target_role.to_string(),
        actor: actor.to_string(),
    })
}

fn load_feedback(
    ws: &WritableStore,
    fb_node: &NamedNode,
) -> Result<crate::core::feedback::Feedback, RouteError> {
    get(&ws.store, fb_node).map_err(|e| match e {
        crate::core::feedback::FeedbackReadError::NotFound { iri } => RouteError::NotFound(iri),
        other => RouteError::Other(format!("{other}")),
    })
}

fn ensure_produced_state(
    ws: &WritableStore,
    fb_node: &NamedNode,
    feedback_iri: &str,
) -> Result<(), RouteError> {
    let prior =
        read_prior_state(&ws.store, fb_node).map_err(|e| RouteError::Other(format!("{e}")))?;
    if prior != LifecycleState::Produced {
        return Err(RouteError::WrongState {
            feedback: feedback_iri.to_string(),
            state: prior.as_str().to_string(),
        });
    }
    Ok(())
}

fn parse_class(
    fb: &crate::core::feedback::Feedback,
    feedback_iri: &str,
) -> Result<FeedbackClass, RouteError> {
    FeedbackClass::from_iri_value(&fb.class).ok_or_else(|| RouteError::UnknownClass {
        feedback: feedback_iri.to_string(),
        class: fb.class.clone(),
    })
}

fn ensure_override_permitted(class: FeedbackClass, class_literal: &str) -> Result<(), RouteError> {
    if !override_permitted_for(class, OverrideActor::Human) {
        return Err(RouteError::OverrideNotPermitted {
            class: class_literal.to_string(),
            actor: OverrideActor::Human,
        });
    }
    Ok(())
}

// Validate the override role exists in the catalog. The routing
// handler ALSO validates this (defensive against direct graph
// edits), but failing fast at the CLI surface gives the operator
// a structured error instead of a silent "unknown-target-role"
// rejection on the next routing pass.
fn ensure_target_role_exists(ws: &WritableStore, target_role: &str) -> Result<(), RouteError> {
    let role = lookup_role(&ws.store, target_role)
        .map_err(|e| RouteError::Other(format!("role catalog lookup: {e:#}")))?;
    if role.is_none() {
        return Err(RouteError::UnknownTargetRole(target_role.to_string()));
    }
    Ok(())
}

fn apply_override(
    ws: &WritableStore,
    fb_node: &NamedNode,
    target_role: &str,
    actor: &str,
) -> Result<(), RouteError> {
    let g = orchestration_graph().into_owned().into();
    let prior_overrides = collect_prior_overrides(&ws.store, fb_node);
    let inserts: Vec<Quad> = vec![
        Quad::new(
            fb_node.clone(),
            routing_override().into_owned(),
            Literal::new_simple_literal(target_role),
            <oxigraph::model::GraphName as Clone>::clone(&g),
        ),
        Quad::new(
            fb_node.clone(),
            routing_override_actor().into_owned(),
            Literal::new_simple_literal(actor),
            g,
        ),
    ];
    let mutation = Mutation {
        inserts,
        removes: prior_overrides,
        ..Mutation::default()
    }
    .with_cause(format!(
        "FT-033 dec feedback route {} --to {target_role} (actor {actor})",
        fb_node.as_str()
    ));
    ws.writer
        .commit(mutation)
        .map_err(|e| RouteError::Other(format!("committing routing override: {e:#}")))?;
    Ok(())
}

fn collect_prior_overrides(store: &Store, fb_node: &NamedNode) -> Vec<Quad> {
    let mut prior: Vec<Quad> = Vec::new();
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(fb_node.clone()).as_ref()),
            None,
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        // Idempotent overrides: remove any prior routingOverride and
        // routingOverrideActor literals so calling `dec feedback route`
        // twice doesn't accumulate stale targets.
        let p = q.predicate.as_str();
        if p == IRI_DEC_ROUTING_OVERRIDE || p == "https://decision-cli.dev/ns#routingOverrideActor"
        {
            prior.push(q);
        }
    }
    prior
}

/// Render a [`RouteOutcome`] as human-readable text.
#[must_use]
pub fn format_route(outcome: &RouteOutcome) -> String {
    format!(
        "routed: {fb}\n  target_role: {role}\n  actor: {actor}\n",
        fb = outcome.feedback_iri,
        role = outcome.target_role,
        actor = outcome.actor,
    )
}

/// Render a [`RouteOutcome`] as JSON.
#[must_use]
pub fn format_route_json(outcome: &RouteOutcome) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"feedback\": \"{}\",\n",
        json_escape(&outcome.feedback_iri)
    ));
    out.push_str(&format!(
        "  \"target_role\": \"{}\",\n",
        json_escape(&outcome.target_role)
    ));
    out.push_str(&format!(
        "  \"actor\": {}\n",
        json_opt(Some(&outcome.actor))
    ));
    out.push_str("}\n");
    out
}

/// Convenience wrapper that produces an `anyhow::Result`.
pub fn route_anyhow(
    workdir: &Path,
    feedback_iri: &str,
    target_role: &str,
    actor: &str,
) -> Result<RouteOutcome> {
    route(workdir, feedback_iri, target_role, actor).map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_actor_falls_back_when_env_clear() {
        // Save & clear standard env vars to ensure deterministic test
        // input. Restore them so we don't break sibling tests.
        let saved: Vec<(&str, Option<String>)> = ["DEC_USER", "USER", "LOGNAME"]
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect();
        for k in ["DEC_USER", "USER", "LOGNAME"] {
            std::env::remove_var(k);
        }
        let actor = resolve_actor();
        assert!(!actor.is_empty());
        // Restore.
        for (k, v) in saved {
            if let Some(val) = v {
                std::env::set_var(k, val);
            }
        }
    }

    #[test]
    fn format_route_text_includes_all_fields() {
        let outcome = RouteOutcome {
            feedback_iri: "urn:f:1".to_string(),
            target_role: "architect".to_string(),
            actor: "alice".to_string(),
        };
        let out = format_route(&outcome);
        assert!(out.contains("routed: urn:f:1"));
        assert!(out.contains("target_role: architect"));
        assert!(out.contains("actor: alice"));
    }

    #[test]
    fn format_route_json_emits_record() {
        let outcome = RouteOutcome {
            feedback_iri: "urn:f:1".to_string(),
            target_role: "architect".to_string(),
            actor: "alice".to_string(),
        };
        let out = format_route_json(&outcome);
        assert!(out.contains("\"feedback\": \"urn:f:1\""));
        assert!(out.contains("\"target_role\": \"architect\""));
        assert!(out.contains("\"actor\": \"alice\""));
    }
}
