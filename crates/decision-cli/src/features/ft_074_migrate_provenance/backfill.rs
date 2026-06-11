//! Synthetic mechanical-triple production (FT-074 §Behaviour step 3).
//!
//! For each backfillable artifact, this module mints:
//!
//!   * a per-artifact `:HistoricalSession` IRI carrying
//!     `:isMigrationBackfill true` + `:migrationNote "<reason>"`,
//!   * a shared `dec:agent:historical-pre-discipline` `:HistoricalAgent`
//!     declared once across the batch (BootstrapArtifact subclass per
//!     FT-074 §Invariants).
//!
//! On the migrated artifact:
//!
//!   * `prov:wasGeneratedBy <historical-session-iri>`
//!   * `prov:wasAttributedTo dec:agent:historical-pre-discipline`
//!   * `prov:generatedAtTime <git-first-commit-timestamp | run-timestamp>`
//!
//! The synthetic Session + Agent themselves carry an `:external_origin`
//! literal documenting the migration run, satisfying the BoundaryArtifact
//! `:external_origin` requirement (FT-071) and recursion termination
//! (FT-074 §Invariants).

#![allow(missing_docs)]

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};

use crate::core::ontology::{
    BOOTSTRAP_ARTIFACT, BOUNDARY_ARTIFACT_CLASS, EXTERNAL_ORIGIN_PROP, IS_MIGRATION_BACKFILL_PROP,
    MIGRATION_BACKFILL,
};
use crate::core::vocab::{
    IRI_DEC_GRAPH_ORCHESTRATION, IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
    IRI_PROV_WAS_GENERATED_BY, IRI_XSD_DATE_TIME,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const IRI_XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

/// Shared `HistoricalAgent` IRI — one per system. Recorded on every
/// backfilled artifact's `prov:wasAttributedTo` triple.
pub const HISTORICAL_AGENT_IRI: &str =
    "https://decision-cli.dev/ns/agent/historical-pre-discipline";

/// `dec:HistoricalSession` class IRI — subclass of `:BoundaryArtifact`
/// via `:MigrationBackfill` so the dual-provenance discipline classifies
/// every synthetic session under the BoundaryArtifact branch.
pub const HISTORICAL_SESSION_CLASS: &str = "https://decision-cli.dev/ns#HistoricalSession";

/// `dec:HistoricalAgent` class IRI — the agent that signs every
/// pre-discipline migration backfill (also a BoundaryArtifact subclass).
pub const HISTORICAL_AGENT_CLASS: &str = "https://decision-cli.dev/ns#HistoricalAgent";

/// `dec:migrationNote` predicate — annotation literal on the
/// `:HistoricalSession` documenting why the backfill ran.
pub const IRI_DEC_MIGRATION_NOTE: &str = "https://decision-cli.dev/ns#migrationNote";

/// One synthesised backfill payload — surfaced in the migration report
/// for traceability and audit.
#[derive(Debug, Clone)]
pub struct BackfillPlan {
    /// Subject artifact whose mechanical block is being materialised.
    pub artifact: NamedNode,
    /// Per-artifact synthetic `:HistoricalSession` IRI.
    pub session: NamedNode,
    /// Shared `:HistoricalAgent` IRI (constant within a migration run).
    pub agent: NamedNode,
    /// `xsd:dateTime` literal used for `prov:generatedAtTime`.
    pub generated_at_time: String,
    /// Free-form note recorded on the synthetic session.
    pub migration_note: String,
}

/// Build a stable `:HistoricalSession` IRI for the artifact +
/// migration run combination. Deterministic so re-running the migration
/// on the same artifact at the same run timestamp produces the same
/// session IRI — required for idempotence.
#[must_use]
pub fn historical_session_iri(artifact: &str, run_id: &str) -> NamedNode {
    let suffix = encode_path_safe(artifact);
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/session/historical-{run_id}-{suffix}"
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

/// Construct the `BackfillPlan` for an artifact. Slice-1 uses the
/// migration run timestamp as the generated-at-time fallback when
/// git-first-commit lookup is not available (FT-074 §Error handling).
#[must_use]
pub fn plan_backfill(
    artifact: &NamedNode,
    run_id: &str,
    generated_at_time: &str,
    migration_note: &str,
) -> BackfillPlan {
    BackfillPlan {
        artifact: artifact.clone(),
        session: historical_session_iri(artifact.as_str(), run_id),
        agent: NamedNode::new_unchecked(HISTORICAL_AGENT_IRI),
        generated_at_time: generated_at_time.to_string(),
        migration_note: migration_note.to_string(),
    }
}

/// Emit every quad the backfill needs for one artifact — the synthetic
/// session block plus the three mechanical-block triples on the artifact.
/// The shared `:HistoricalAgent` declaration is emitted separately via
/// [`emit_shared_agent_quads`] so the per-artifact call sites stay tight.
#[must_use]
pub fn emit_backfill_quads(plan: &BackfillPlan, run_external_origin: &str) -> Vec<Quad> {
    let g: GraphName = orchestration_graph();
    let mut quads = synthetic_session_quads(plan, run_external_origin, &g);
    quads.extend(mechanical_block_quads(plan, &g));
    quads
}

/// Synthetic `:HistoricalSession` declaration — class membership,
/// `:isMigrationBackfill true`, `:external_origin`, and the migration note.
fn synthetic_session_quads(
    plan: &BackfillPlan,
    run_external_origin: &str,
    g: &GraphName,
) -> Vec<Quad> {
    vec![
        typed_quad(
            plan.session.clone(),
            RDF_TYPE,
            HISTORICAL_SESSION_CLASS,
            g.clone(),
        ),
        typed_quad(
            plan.session.clone(),
            RDF_TYPE,
            BOUNDARY_ARTIFACT_CLASS,
            g.clone(),
        ),
        typed_quad(
            plan.session.clone(),
            RDF_TYPE,
            MIGRATION_BACKFILL,
            g.clone(),
        ),
        boolean_quad(
            plan.session.clone(),
            IS_MIGRATION_BACKFILL_PROP,
            true,
            g.clone(),
        ),
        literal_quad(
            plan.session.clone(),
            EXTERNAL_ORIGIN_PROP,
            run_external_origin,
            g.clone(),
        ),
        literal_quad(
            plan.session.clone(),
            IRI_DEC_MIGRATION_NOTE,
            &plan.migration_note,
            g.clone(),
        ),
    ]
}

/// Three mechanical-block triples on the migrated artifact.
fn mechanical_block_quads(plan: &BackfillPlan, g: &GraphName) -> Vec<Quad> {
    vec![
        named_quad(
            plan.artifact.clone(),
            IRI_PROV_WAS_GENERATED_BY,
            plan.session.clone(),
            g.clone(),
        ),
        named_quad(
            plan.artifact.clone(),
            IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
            plan.agent.clone(),
            g.clone(),
        ),
        datetime_quad(
            plan.artifact.clone(),
            IRI_PROV_GENERATED_AT_TIME,
            &plan.generated_at_time,
            g.clone(),
        ),
    ]
}

/// Emit the once-per-run declaration of the shared `:HistoricalAgent`.
/// The agent is itself a `BoundaryArtifact / BootstrapArtifact` (per
/// FT-074 §Behaviour step 3 — "This Agent is itself a BoundaryArtifact
/// of subclass BootstrapArtifact").
#[must_use]
pub fn emit_shared_agent_quads(run_external_origin: &str) -> Vec<Quad> {
    let g: GraphName = orchestration_graph();
    let agent = NamedNode::new_unchecked(HISTORICAL_AGENT_IRI);
    vec![
        typed_quad(agent.clone(), RDF_TYPE, HISTORICAL_AGENT_CLASS, g.clone()),
        typed_quad(agent.clone(), RDF_TYPE, BOUNDARY_ARTIFACT_CLASS, g.clone()),
        typed_quad(agent.clone(), RDF_TYPE, BOOTSTRAP_ARTIFACT, g.clone()),
        literal_quad(agent, EXTERNAL_ORIGIN_PROP, run_external_origin, g),
    ]
}

// ---------------------------------------------------------------------------
// quad helpers — kept private; not part of the public surface.
// ---------------------------------------------------------------------------

fn orchestration_graph() -> GraphName {
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

fn typed_quad(subject: NamedNode, predicate: &str, object: &str, g: GraphName) -> Quad {
    Quad::new(
        subject,
        NamedNode::new_unchecked(predicate),
        NamedNode::new_unchecked(object),
        g,
    )
}

fn named_quad(subject: NamedNode, predicate: &str, object: NamedNode, g: GraphName) -> Quad {
    Quad::new(subject, NamedNode::new_unchecked(predicate), object, g)
}

fn literal_quad(subject: NamedNode, predicate: &str, value: &str, g: GraphName) -> Quad {
    Quad::new(
        subject,
        NamedNode::new_unchecked(predicate),
        Literal::new_simple_literal(value),
        g,
    )
}

fn boolean_quad(subject: NamedNode, predicate: &str, value: bool, g: GraphName) -> Quad {
    Quad::new(
        subject,
        NamedNode::new_unchecked(predicate),
        Literal::new_typed_literal(
            if value { "true" } else { "false" },
            NamedNode::new_unchecked(IRI_XSD_BOOLEAN),
        ),
        g,
    )
}

fn datetime_quad(subject: NamedNode, predicate: &str, value: &str, g: GraphName) -> Quad {
    Quad::new(
        subject,
        NamedNode::new_unchecked(predicate),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(IRI_XSD_DATE_TIME)),
        g,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_session_iri_is_deterministic() {
        let a = "https://decision-cli.dev/ns/adr/ADR-001";
        let s1 = historical_session_iri(a, "run-1");
        let s2 = historical_session_iri(a, "run-1");
        assert_eq!(s1, s2);
        let s3 = historical_session_iri(a, "run-2");
        assert_ne!(s1, s3);
    }

    #[test]
    fn emit_backfill_emits_mechanical_and_session_quads() {
        let artifact = NamedNode::new_unchecked("https://decision-cli.dev/ns/adr/ADR-001");
        let plan = plan_backfill(
            &artifact,
            "run-1",
            "2026-05-25T20:30:00Z",
            "FT-074 backfill",
        );
        let quads = emit_backfill_quads(
            &plan,
            "FT-074 provenance migration tool run at 2026-05-25T20:30:00Z",
        );
        // At least: 3 mechanical + 3 type triples on session + isMigrationBackfill + external_origin + migrationNote = 9
        assert!(quads.len() >= 9, "expected >= 9 quads, got {}", quads.len());
        let preds: Vec<&str> = quads.iter().map(|q| q.predicate.as_str()).collect();
        assert!(preds.contains(&IRI_PROV_WAS_GENERATED_BY));
        assert!(preds.contains(&IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL));
        assert!(preds.contains(&IRI_PROV_GENERATED_AT_TIME));
        assert!(preds.contains(&IS_MIGRATION_BACKFILL_PROP));
        assert!(preds.contains(&EXTERNAL_ORIGIN_PROP));
        assert!(preds.contains(&IRI_DEC_MIGRATION_NOTE));
    }
}
