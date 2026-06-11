//! Feedback emission for unrepairable migration orphans (FT-074 §Behaviour step 4).
//!
//! For every artifact the audit classifies as `Orphan`, this module emits:
//!
//!   * a `dec:Feedback` artifact of class `migration-orphan-needs-repair`
//!     routed to the operator-curator target role,
//!   * an `:isMigrationOrphan true` annotation on the orphan artifact
//!     itself so write-time warnings can detect it.
//!
//! Re-running migration is idempotent (FT-074 §Behaviour step 7): we
//! derive a deterministic feedback IRI from the (run_id, artifact)
//! pair so the same orphan re-emitted at the same run produces the same
//! IRI. To handle the cross-run idempotence guarantee promised by TC-124
//! (no duplicate feedback emissions across runs), callers also check
//! whether a feedback for that artifact already exists in the store
//! before emitting (see [`feedback_already_exists_for_orphan`]).

#![allow(missing_docs)]

use anyhow::Result;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::vocab::{
    feedback_class as feedback_iri_node, feedback_class_pred, lifecycle_state, severity,
    source_artifact, target_role, IRI_DEC_GRAPH_ORCHESTRATION, IRI_DEC_RECOMMENDATION,
};

use super::mapping::orphan_repair_guidance;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const IRI_XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

/// Slice-1 feedback class for migration orphans. Distinct from FT-073's
/// `provenance-violation` class because this emission is the migration
/// tool flagging *existing* artifacts rather than the validator
/// rejecting a *new* write.
pub const MIGRATION_ORPHAN_FEEDBACK_CLASS: &str = "migration-orphan-needs-repair";

/// Default target role per FT-074 §Outputs ("routed to the operator-
/// curator role per FT-029"). Override-able via the CLI in a slice-2+
/// rework, but slice 1 hardcodes.
pub const ORPHAN_TARGET_ROLE: &str = "operator-curator";

/// Severity literal stamped on every orphan feedback.
pub const ORPHAN_SEVERITY: &str = "non-blocking";

/// `:isMigrationOrphan` predicate IRI — annotates the orphan artifact
/// itself so write-time warnings can detect it.
pub const IRI_DEC_IS_MIGRATION_ORPHAN: &str = "https://decision-cli.dev/ns#isMigrationOrphan";

/// Plan one orphan emission. Returned by the audit-to-emit pipeline so
/// callers can apply or render without invoking the writer.
#[derive(Debug, Clone)]
pub struct OrphanFeedbackPlan {
    /// IRI of the orphan artifact being flagged.
    pub orphan_artifact: NamedNode,
    /// IRI of the feedback artifact about to be emitted.
    pub feedback_iri: NamedNode,
    /// Stable run identifier used to derive `feedback_iri`.
    pub run_id: String,
    /// Per-type repair guidance (mirrors FT-070's vocabulary table).
    pub recommendation: String,
    /// Reasons the audit classified the artifact as orphan.
    pub reasons: Vec<String>,
}

/// Build a deterministic feedback IRI for an orphan in a given run.
#[must_use]
pub fn orphan_feedback_iri(orphan_artifact: &str, run_id: &str) -> NamedNode {
    let suffix = encode_path_safe(orphan_artifact);
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/feedback/migration-orphan-{run_id}-{suffix}"
    ))
}

fn encode_path_safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Build the feedback plan from an audit verdict's facts.
#[must_use]
pub fn plan_orphan_feedback(
    orphan_artifact: &NamedNode,
    rdf_type: &str,
    reasons: &[String],
    run_id: &str,
) -> OrphanFeedbackPlan {
    OrphanFeedbackPlan {
        orphan_artifact: orphan_artifact.clone(),
        feedback_iri: orphan_feedback_iri(orphan_artifact.as_str(), run_id),
        run_id: run_id.to_string(),
        recommendation: orphan_repair_guidance(rdf_type),
        reasons: reasons.to_vec(),
    }
}

/// Emit the full quad set for an orphan feedback: the `dec:Feedback`
/// artifact itself plus the `:isMigrationOrphan true` annotation on the
/// orphan.
#[must_use]
pub fn emit_orphan_feedback_quads(plan: &OrphanFeedbackPlan) -> Vec<Quad> {
    let g: GraphName = orchestration_graph();
    let mut quads = required_feedback_quads(&plan.feedback_iri, &g);
    quads.push(routing_quad(&plan.feedback_iri, &g));
    quads.push(severity_quad(&plan.feedback_iri, &g));
    quads.push(source_artifact_quad(
        &plan.feedback_iri,
        &plan.orphan_artifact,
        &g,
    ));
    quads.push(recommendation_quad(
        &plan.feedback_iri,
        &plan.recommendation,
        &g,
    ));
    quads.push(evidence_quad(&plan.feedback_iri, &plan.reasons, &g));
    quads.push(orphan_marker_quad(&plan.orphan_artifact, &g));
    quads
}

fn required_feedback_quads(feedback_iri: &NamedNode, g: &GraphName) -> Vec<Quad> {
    vec![
        Quad::new(
            feedback_iri.clone(),
            NamedNode::new_unchecked(RDF_TYPE),
            feedback_iri_node().into_owned(),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            feedback_class_pred().into_owned(),
            Literal::new_simple_literal(MIGRATION_ORPHAN_FEEDBACK_CLASS),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            lifecycle_state().into_owned(),
            Literal::new_simple_literal("produced"),
            g.clone(),
        ),
    ]
}

fn routing_quad(feedback_iri: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(
        feedback_iri.clone(),
        target_role().into_owned(),
        Literal::new_simple_literal(ORPHAN_TARGET_ROLE),
        g.clone(),
    )
}

fn severity_quad(feedback_iri: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(
        feedback_iri.clone(),
        severity().into_owned(),
        Literal::new_simple_literal(ORPHAN_SEVERITY),
        g.clone(),
    )
}

fn source_artifact_quad(feedback_iri: &NamedNode, orphan: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(
        feedback_iri.clone(),
        source_artifact().into_owned(),
        orphan.clone(),
        g.clone(),
    )
}

fn recommendation_quad(feedback_iri: &NamedNode, text: &str, g: &GraphName) -> Quad {
    Quad::new(
        feedback_iri.clone(),
        NamedNode::new_unchecked(IRI_DEC_RECOMMENDATION),
        Literal::new_simple_literal(text),
        g.clone(),
    )
}

fn evidence_quad(feedback_iri: &NamedNode, reasons: &[String], g: &GraphName) -> Quad {
    let body = format!(
        "Migration orphan reasons:\n{}",
        reasons
            .iter()
            .map(|r| format!("  • {r}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    Quad::new(
        feedback_iri.clone(),
        crate::core::vocab::evidence().into_owned(),
        Literal::new_simple_literal(body),
        g.clone(),
    )
}

fn orphan_marker_quad(orphan: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(
        orphan.clone(),
        NamedNode::new_unchecked(IRI_DEC_IS_MIGRATION_ORPHAN),
        Literal::new_typed_literal("true", NamedNode::new_unchecked(IRI_XSD_BOOLEAN)),
        g.clone(),
    )
}

fn orchestration_graph() -> GraphName {
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

/// Idempotence guard — returns `true` when the store already has a
/// `migration-orphan-needs-repair` feedback for `orphan_artifact`.
pub fn feedback_already_exists_for_orphan(store: &Store, orphan_artifact: &str) -> Result<bool> {
    let sparql = format!(
        "ASK {{ \
           {{ ?f a <{ft}> ; <{fc}> ?cls ; <{sa}> <{a}> . FILTER (str(?cls) = \"{cls_lit}\") }} \
           UNION \
           {{ GRAPH ?g {{ ?f a <{ft}> ; <{fc}> ?cls ; <{sa}> <{a}> . FILTER (str(?cls) = \"{cls_lit}\") }} }} \
         }}",
        ft = crate::core::vocab::IRI_DEC_FEEDBACK,
        fc = crate::core::vocab::IRI_DEC_FEEDBACK_CLASS,
        sa = crate::core::vocab::IRI_DEC_SOURCE_ARTIFACT,
        a = orphan_artifact,
        cls_lit = MIGRATION_ORPHAN_FEEDBACK_CLASS,
    );
    match store.query(sparql.as_str())? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}

/// Idempotence guard for the orphan annotation itself.
pub fn artifact_already_marked_orphan(store: &Store, orphan_artifact: &str) -> Result<bool> {
    let sparql = format!(
        "ASK {{ \
           {{ <{a}> <{p}> ?v . FILTER(?v = true || str(?v) = \"true\") }} \
           UNION \
           {{ GRAPH ?g {{ <{a}> <{p}> ?v . FILTER(?v = true || str(?v) = \"true\") }} }} \
         }}",
        a = orphan_artifact,
        p = IRI_DEC_IS_MIGRATION_ORPHAN,
    );
    match store.query(sparql.as_str())? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}
