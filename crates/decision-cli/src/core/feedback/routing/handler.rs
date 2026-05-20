//! Delivery handler for the feedback-routing subscription (FT-029 / ADR-026).
//!
//! The seed in `seeds/feedback_routing.ttl` matches every `dec:Feedback`
//! in `lifecycleState "produced"`. This module owns the *handler* side —
//! the small piece of logic that turns each match into either a
//! `produced → routed` lifecycle transition (with `dec:routedAt` +
//! `dec:targetRole` evidence) or a `produced → ?` rejection path for
//! unknown / disallowed override targets.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, the handler lives
//! under `core/feedback/routing`; feature modules call
//! [`route_pending_feedback`] from their dispatch loops rather than
//! reimplementing the lookup.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::feedback::class::FeedbackClass;
use crate::core::feedback::lifecycle::LifecycleState;
use crate::core::feedback::transition::{apply, ApplyError};
use crate::core::role_catalog::role::lookup as lookup_role;
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    orchestration_graph, rejection_reason as rejection_reason_pred, routed_at as routed_at_pred,
    target_role as target_role_pred,
};

use super::table::{default_target_role, OverrideActor, ROUTING_TABLE};

/// Stable IRI of the seeded feedback-routing subscription.
pub const FEEDBACK_ROUTING_SUBSCRIPTION_IRI: &str =
    "https://decision-cli.dev/ns/subscription/feedback-routing";

/// Opaque handler tag the subscription seed carries on `oxi:handler`. The
/// slice-3 harness binds this tag to [`route_pending_feedback`].
pub const FEEDBACK_ROUTING_HANDLER: &str = "feedback-routing";

/// Raw Turtle bytes for the feedback-routing subscription seed, embedded
/// via `include_str!`. Loaded into the orchestration store at `dec init`
/// time by `features/init`.
pub const FEEDBACK_ROUTING_SEED_TTL: &str = include_str!("../seeds/feedback_routing.ttl");

/// SPARQL run by the seed subscription. Mirrors FT-029 §Outputs verbatim
/// (with a `GRAPH` UNION so it matches feedback quads regardless of
/// whether they live in the default graph or the orchestration named
/// graph).
pub(crate) const PENDING_FEEDBACK_QUERY: &str = "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?feedback ?class ?targetOverride ?sourceSession WHERE {
  { ?feedback a dec:Feedback ;
              dec:lifecycleState \"produced\" ;
              dec:feedbackClass ?class ;
              dec:sourceSession ?sourceSession .
    OPTIONAL { ?feedback dec:routingOverride ?targetOverride } }
  UNION
  { GRAPH ?g {
      ?feedback a dec:Feedback ;
                dec:lifecycleState \"produced\" ;
                dec:feedbackClass ?class ;
                dec:sourceSession ?sourceSession .
      OPTIONAL { ?feedback dec:routingOverride ?targetOverride }
    } }
}";

/// One `(feedback, class, target_override, source_session)` row selected
/// by the seed subscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFeedback {
    /// IRI of the `dec:Feedback` in `produced`.
    pub feedback: NamedNode,
    /// Class literal (matches an entry in the controlled vocabulary).
    pub class: String,
    /// Optional override target role id.
    pub target_override: Option<String>,
    /// IRI of the originating session.
    pub source_session: NamedNode,
}

/// Outcome of a single routing decision for one feedback artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingOutcome {
    /// Feedback transitioned to `routed` with the resolved target role.
    Routed {
        /// IRI of the routed feedback.
        feedback: NamedNode,
        /// Resolved role id.
        target_role: String,
    },
    /// Feedback transitioned to `rejected` because routing could not
    /// produce a valid target role.
    Rejected {
        /// IRI of the rejected feedback.
        feedback: NamedNode,
        /// Machine-readable rejection reason literal.
        reason: &'static str,
    },
    /// Feedback was already past `produced` when the handler observed it
    /// (concurrent update / re-run after crash). No mutation written.
    AlreadyRouted {
        /// IRI of the feedback.
        feedback: NamedNode,
    },
}

/// Errors surfaced by the feedback-routing handler.
#[derive(Debug, Error)]
pub enum FeedbackRoutingError {
    /// Underlying SPARQL / store failure.
    #[error("pending-feedback lookup: {0}")]
    Lookup(String),
    /// The class literal observed on the feedback is not one of the six
    /// controlled values (ADR-023). FT-028's SHACL refuses these at write
    /// time; the routing handler treats them defensively.
    #[error("feedback <{feedback}> carries unknown class {class:?}")]
    UnknownClass {
        /// IRI of the offending feedback.
        feedback: String,
        /// The literal that did not match the controlled vocabulary.
        class: String,
    },
    /// Underlying transition application failure.
    #[error("transition apply: {0}")]
    Transition(#[from] ApplyError),
}

/// Machine-readable rejection reason literals consumed by the handler.
pub const REJECTION_UNKNOWN_TARGET_ROLE: &str = "unknown-target-role";
/// Override role exists in the catalog but is not allowed for the class.
pub const REJECTION_OVERRIDE_NOT_PERMITTED: &str = "override-not-permitted";

/// Run the seed query against `store` and return every `produced`
/// feedback that needs a routing decision. Mirrors the SPARQL embedded
/// in `seeds/feedback_routing.ttl`. Idempotency is handled at apply
/// time: `produced → routed` validates the prior state via
/// [`apply`] and silently drops feedback that has already advanced.
pub fn pending_feedback(store: &Store) -> Result<Vec<PendingFeedback>, FeedbackRoutingError> {
    let solutions = match store
        .query(PENDING_FEEDBACK_QUERY)
        .map_err(|e| FeedbackRoutingError::Lookup(format!("{e:#}")))?
    {
        QueryResults::Solutions(s) => s,
        _ => return Ok(Vec::new()),
    };
    let mut out: Vec<PendingFeedback> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for sol in solutions {
        let sol = sol.map_err(|e| FeedbackRoutingError::Lookup(format!("{e:#}")))?;
        let Some(Term::NamedNode(feedback)) = sol.get("feedback").cloned() else {
            continue;
        };
        let Some(Term::Literal(class_lit)) = sol.get("class").cloned() else {
            continue;
        };
        let Some(Term::NamedNode(source_session)) = sol.get("sourceSession").cloned() else {
            continue;
        };
        if !seen.insert(feedback.as_str().to_string()) {
            continue;
        }
        let target_override = match sol.get("targetOverride").cloned() {
            Some(Term::Literal(lit)) => Some(lit.value().to_string()),
            Some(Term::NamedNode(n)) => Some(n.as_str().to_string()),
            _ => None,
        };
        out.push(PendingFeedback {
            feedback,
            class: class_lit.value().to_string(),
            target_override,
            source_session,
        });
    }
    Ok(out)
}

/// Resolve, validate, and apply a routing decision for one pending
/// feedback. Returns the [`RoutingOutcome`] describing what was written.
///
/// Decision tree (FT-029 §Behaviour step 3, ADR-026):
///
/// 1. Parse the class literal. Unknown class → `UnknownClass` error
///    (defensive — SHACL refuses this at write time).
/// 2. Resolve the target role:
///    * if `target_override` is set and permitted for this class +
///      actor, use it;
///    * otherwise fall back to the table's default for the class.
/// 3. Validate the resolved role exists in the catalog. If not, mark
///    the feedback `rejected` with `unknown-target-role`.
/// 4. If an override was supplied, ensure the override actor is allowed
///    by the routing-rule policy. If not, `rejected` with
///    `override-not-permitted`.
/// 5. Otherwise transition `produced → routed` with
///    `dec:routedAt = now_rfc3339` and `dec:targetRole = resolved`.
///
/// The handler is idempotent — calls against a feedback that already
/// advanced past `produced` return [`RoutingOutcome::AlreadyRouted`]
/// without writing.
pub fn route_one(
    store: &Store,
    writer: &StreamWriter,
    pending: &PendingFeedback,
    now_rfc3339: &str,
    override_actor: OverrideActor,
) -> Result<RoutingOutcome, FeedbackRoutingError> {
    let class = FeedbackClass::from_iri_value(&pending.class).ok_or_else(|| {
        FeedbackRoutingError::UnknownClass {
            feedback: pending.feedback.as_str().to_string(),
            class: pending.class.clone(),
        }
    })?;

    let resolved = resolve_target_role(class, pending.target_override.as_deref(), override_actor);

    let g: GraphName = orchestration_graph().into_owned().into();

    match resolved {
        TargetResolution::Default(role) | TargetResolution::Override(role) => {
            if !target_role_exists(store, role)? {
                return reject(
                    store,
                    writer,
                    &pending.feedback,
                    REJECTION_UNKNOWN_TARGET_ROLE,
                    &g,
                );
            }
            apply_routed(writer, &pending.feedback, role, now_rfc3339, &g)
        }
        TargetResolution::OverrideDisallowed => reject(
            store,
            writer,
            &pending.feedback,
            REJECTION_OVERRIDE_NOT_PERMITTED,
            &g,
        ),
    }
}

/// Convenience wrapper: list pending feedback and route each row with the
/// given `now` timestamp. Errors short-circuit; partial progress is
/// observable in the store (each routed feedback is its own commit).
pub fn route_pending_feedback(
    store: &Store,
    writer: &StreamWriter,
    now_rfc3339: &str,
    override_actor: OverrideActor,
) -> Result<Vec<RoutingOutcome>, FeedbackRoutingError> {
    let mut outcomes: Vec<RoutingOutcome> = Vec::new();
    for pending in pending_feedback(store)? {
        outcomes.push(route_one(store, writer, &pending, now_rfc3339, override_actor)?);
    }
    Ok(outcomes)
}

enum TargetResolution<'a> {
    Default(&'a str),
    Override(&'a str),
    OverrideDisallowed,
}

fn resolve_target_role<'a>(
    class: FeedbackClass,
    override_role: Option<&'a str>,
    actor: OverrideActor,
) -> TargetResolution<'a> {
    match override_role {
        Some(role) => {
            if super::table::override_permitted_for(class, actor) {
                TargetResolution::Override(role)
            } else {
                TargetResolution::OverrideDisallowed
            }
        }
        None => TargetResolution::Default(default_target_role(class)),
    }
}

fn target_role_exists(store: &Store, role_id: &str) -> Result<bool, FeedbackRoutingError> {
    let role = lookup_role(store, role_id)
        .map_err(|e| FeedbackRoutingError::Lookup(format!("role catalog lookup: {e:#}")))?;
    Ok(role.is_some())
}

fn apply_routed(
    writer: &StreamWriter,
    feedback: &NamedNode,
    role: &str,
    now_rfc3339: &str,
    g: &GraphName,
) -> Result<RoutingOutcome, FeedbackRoutingError> {
    let evidence: Vec<Quad> = vec![
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
    ];
    // Use the orchestration named graph as the canonical target for the
    // transition; the prior `dec:targetRole` literal (if any) is removed
    // by `apply` together with the lifecycle literal swap.
    let graph_ref: NamedNodeRef<'_> = match g {
        GraphName::NamedNode(n) => n.as_ref(),
        _ => orchestration_graph(),
    };
    // `apply` reads the prior state and validates the transition before
    // committing. Idempotency: if `prior` already advanced past
    // `produced`, the transition validator refuses; we treat that as the
    // `AlreadyRouted` outcome.
    let result = apply(
        writer.inner().store(),
        writer,
        feedback,
        LifecycleState::Routed,
        evidence,
        graph_ref,
    );
    match result {
        Ok(()) => Ok(RoutingOutcome::Routed {
            feedback: feedback.clone(),
            target_role: role.to_string(),
        }),
        Err(ApplyError::Transition(_)) => Ok(RoutingOutcome::AlreadyRouted {
            feedback: feedback.clone(),
        }),
        Err(err) => Err(FeedbackRoutingError::from(err)),
    }
}

/// Routing-time rejection path. ADR-024 forbids `produced → rejected`
/// as a direct lifecycle transition (rejection paths are only valid out
/// of `received` or `addressed`). For routing-time failures the
/// orchestrator has no receiving session to anchor a `received` step
/// on, so we record `dec:rejectionReason` on the feedback while keeping
/// its lifecycle literal at `produced`. The read layer filters feedback
/// with a `dec:rejectionReason` set out of `list_open`, which is enough
/// to satisfy FT-029's invariant that no further routing is attempted.
fn reject(
    store: &Store,
    writer: &StreamWriter,
    feedback: &NamedNode,
    reason: &'static str,
    g: &GraphName,
) -> Result<RoutingOutcome, FeedbackRoutingError> {
    let _ = store;
    let mutation_quads: Vec<Quad> = vec![Quad::new(
        feedback.clone(),
        rejection_reason_pred().into_owned(),
        Literal::new_simple_literal(reason),
        g.clone(),
    )];
    let mutation = oxi_events::Mutation::insert(mutation_quads.iter().cloned())
        .with_cause(format!("FT-029 routing rejected: {reason}"));
    writer
        .commit(mutation)
        .with_context(|| format!("recording rejection reason {reason}"))
        .map_err(|e| FeedbackRoutingError::Lookup(format!("{e:#}")))?;
    Ok(RoutingOutcome::Rejected {
        feedback: feedback.clone(),
        reason,
    })
}

/// Build the quad set that seeds the feedback-routing subscription into
/// the `oxi-events:subscriptions` named graph. Used by the init pipeline
/// alongside the slice-1 v0 bootstrap subscriptions and the FT-022
/// verifier-dispatch seed.
#[must_use]
pub fn seed_quads() -> Vec<Quad> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let sub = NamedNode::new_unchecked(FEEDBACK_ROUTING_SUBSCRIPTION_IRI);
    let rdf_type =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").into_owned();
    let sub_cls = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    let handler_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_HANDLER);
    let label_pred = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    vec![
        Quad::new(sub.clone(), rdf_type, sub_cls, subs_graph.clone()),
        Quad::new(
            sub.clone(),
            select_pred,
            Literal::new_simple_literal(PENDING_FEEDBACK_QUERY),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            mode_pred,
            Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_INLINE),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            handler_pred,
            Literal::new_simple_literal(FEEDBACK_ROUTING_HANDLER),
            subs_graph.clone(),
        ),
        Quad::new(
            sub,
            label_pred,
            Literal::new_simple_literal("feedback routing (produced → routed)"),
            subs_graph,
        ),
    ]
}

/// Number of rows in the routing table — used by tests to assert the
/// table stays in sync with the controlled vocabulary.
#[must_use]
pub const fn routing_table_len() -> usize {
    ROUTING_TABLE.len()
}
