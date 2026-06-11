//! Persist `dec:SessionRecord` per cluster cell + parent
//! `dec:ClusterDispatch` activity (FT-146 / ADR-080 / FT-057).
//!
//! `cluster_dispatch::run` accumulates per-cell `CellSessionRecord`
//! values during dispatch — one per cell, mechanical or LLM-backed —
//! then calls [`persist_cluster_run`] once at the end. That keeps the
//! mutation atomic with respect to mid-flight observers: either the
//! parent activity has its `prov:endedAtTime` set (the dispatch is
//! complete) or the activity has not been written yet. No half-closed
//! clusters land in the graph.
//!
//! The cell session uses FT-057's [`SessionRecord`] for the four
//! token-breakdown fields + the `dec:capability` link, plus a small
//! framing of extra quads for status, timing, the cluster-parent
//! `prov:wasInformedBy` link, and the FT-146 `dec:usageSource`
//! provenance. Scaleway dispatches zero-fill the cache fields per
//! FT-057 §SHACL — the worker already does so via its zero defaults
//! on `WorkerResponseUsage` for Scaleway.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::ontology::session_record::SessionRecord;
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::orchestration_graph;
use crate::features::implement::WorkerResponseUsage;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
const PROV_WAS_INFORMED_BY: &str = "http://www.w3.org/ns/prov#wasInformedBy";
const PROV_STARTED_AT_TIME: &str = "http://www.w3.org/ns/prov#startedAtTime";
const PROV_ENDED_AT_TIME: &str = "http://www.w3.org/ns/prov#endedAtTime";

/// `dec:ClusterDispatch` — the parent activity that groups a cluster's
/// per-cell sessions (FT-146).
pub const IRI_DEC_CLUSTER_DISPATCH: &str = "https://decision-cli.dev/ns#ClusterDispatch";

/// `dec:cellStatus` — string literal: `succeeded` | `failed` | `mechanical`.
pub const IRI_DEC_CELL_STATUS: &str = "https://decision-cli.dev/ns#cellStatus";

/// `dec:usageSource` — string literal: `worker-reported` | `litellm-telemetry` | `unreported`.
pub const IRI_DEC_USAGE_SOURCE: &str = "https://decision-cli.dev/ns#usageSource";

/// `dec:clusterOutcome` — string literal on the cluster activity:
/// `succeeded` | `audit_failed` | `cell_failed` | `audit_unrunnable`.
pub const IRI_DEC_CLUSTER_OUTCOME: &str = "https://decision-cli.dev/ns#clusterOutcome";

/// Per-cell session record accumulated during cluster dispatch.
#[derive(Debug, Clone)]
pub struct CellSessionRecord {
    /// Stable cell session IRI — matches `cluster_dispatch`'s synthetic
    /// IRI convention `urn:dec:cluster-session:<task-type>/<feature>/<cell>`.
    pub iri: NamedNode,
    /// Capability the cell resolved against. For mechanical cells this is
    /// a synthetic `urn:dec:capability:mechanical` IRI so the
    /// `dec:capability` link is uniform across LLM-backed and mechanical
    /// cells.
    pub capability: NamedNode,
    /// Per-call accumulated token usage. `None` when no LLM call surfaced
    /// a usage block — the harness records `dec:usageSource = "unreported"`.
    pub usage: Option<WorkerResponseUsage>,
    /// Cell outcome: `succeeded` (LLM cell wrote its output and the
    /// worker exited cleanly), `failed` (worker errored before / during
    /// the cell write), or `mechanical` (template-rendered, no LLM).
    pub status: CellStatus,
    /// RFC-3339 timestamp recorded immediately before the cell dispatch.
    pub started_at: DateTime<Utc>,
    /// RFC-3339 timestamp recorded immediately after the cell dispatch.
    pub ended_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStatus {
    Succeeded,
    Failed,
    Mechanical,
}

impl CellStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Mechanical => "mechanical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterOutcome {
    Succeeded,
    AuditFailed,
    CellFailed,
    AuditUnrunnable,
}

impl ClusterOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::AuditFailed => "audit_failed",
            Self::CellFailed => "cell_failed",
            Self::AuditUnrunnable => "audit_unrunnable",
        }
    }
}

/// Persist the parent `dec:ClusterDispatch` activity + every per-cell
/// `dec:SessionRecord` in a single mutation, then write the store back
/// to disk. Idempotent persistence is best-effort: a downstream failure
/// (SHACL rejection, disk write error) bubbles up as an error, but the
/// caller's primary error (audit fail, cell fail) takes precedence.
#[allow(clippy::too_many_arguments)]
pub fn persist_cluster_run(
    workdir: &Path,
    cluster_iri: &NamedNode,
    feature_id: &str,
    task_type_name: &str,
    cluster_started: DateTime<Utc>,
    cluster_ended: DateTime<Utc>,
    outcome: ClusterOutcome,
    cells: &[CellSessionRecord],
) -> Result<()> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)
        .with_context(|| format!("opening orchestration store at {}", dump.display()))?;
    let store = Arc::new(store);

    let scope = ActiveScope::load(workdir).context("loading active scope for cluster session")?;
    let stream_iri =
        NamedNode::new(&scope.stream_iri).context("active stream iri for cluster session")?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri.clone())
        .context("opening writer for cluster session persist")?;

    let g: GraphName = orchestration_graph().into_owned().into();
    let mut quads: Vec<Quad> = Vec::new();

    // Parent cluster activity.
    quads.extend(cluster_activity_quads(
        cluster_iri,
        feature_id,
        task_type_name,
        cluster_started,
        cluster_ended,
        outcome,
        &g,
    ));

    // Per-cell session records.
    for cell in cells {
        quads.extend(cell_session_quads(cluster_iri, cell, &g));
    }

    writer
        .commit(Mutation::insert(quads))
        .context("committing cluster session quads through StreamWriter")?;

    persist_store(&store, &dump)
        .context("persisting orchestration store after cluster session write")?;
    Ok(())
}

fn cluster_activity_quads(
    cluster_iri: &NamedNode,
    feature_id: &str,
    task_type_name: &str,
    started: DateTime<Utc>,
    ended: DateTime<Utc>,
    outcome: ClusterOutcome,
    g: &GraphName,
) -> Vec<Quad> {
    vec![
        named_quad_typed(
            cluster_iri,
            RDF_TYPE,
            NamedNode::new_unchecked(PROV_ACTIVITY),
            g,
        ),
        named_quad_typed(
            cluster_iri,
            RDF_TYPE,
            NamedNode::new_unchecked(IRI_DEC_CLUSTER_DISPATCH),
            g,
        ),
        literal_quad(cluster_iri, PROV_STARTED_AT_TIME, &started.to_rfc3339(), g),
        literal_quad(cluster_iri, PROV_ENDED_AT_TIME, &ended.to_rfc3339(), g),
        literal_quad(cluster_iri, IRI_DEC_CLUSTER_OUTCOME, outcome.as_str(), g),
        literal_quad(
            cluster_iri,
            "https://decision-cli.dev/ns#featureId",
            feature_id,
            g,
        ),
        literal_quad(
            cluster_iri,
            "https://decision-cli.dev/ns#taskType",
            task_type_name,
            g,
        ),
    ]
}

fn cell_session_quads(
    cluster_iri: &NamedNode,
    cell: &CellSessionRecord,
    g: &GraphName,
) -> Vec<Quad> {
    // Build the FT-057 SessionRecord (rdf:type, capability, four token
    // counts) — passes the FT-057 SHACL validator at the chokepoint.
    let usage_source = if cell.usage.is_some() {
        "worker-reported"
    } else {
        "unreported"
    };
    let usage = cell.usage.clone().unwrap_or_default();

    let session_record = SessionRecord {
        iri: cell.iri.clone(),
        escalated_from: None,
        escalation_reason: None,
        input_tokens_base: usage.input_tokens_base,
        input_tokens_cache_write: usage.input_tokens_cache_write,
        input_tokens_cache_hit: usage.input_tokens_cache_hit,
        output_tokens: usage.output_tokens,
        capability: cell.capability.clone(),
    };

    let graph_ref = orchestration_graph();
    let mut quads = session_record.to_quads(graph_ref);

    // FT-146 framing: parent-cluster link, status, timing, usage_source.
    quads.push(named_quad_typed(
        &cell.iri,
        PROV_WAS_INFORMED_BY,
        cluster_iri.clone(),
        g,
    ));
    quads.push(literal_quad(
        &cell.iri,
        IRI_DEC_CELL_STATUS,
        cell.status.as_str(),
        g,
    ));
    quads.push(literal_quad(
        &cell.iri,
        IRI_DEC_USAGE_SOURCE,
        usage_source,
        g,
    ));
    quads.push(literal_quad(
        &cell.iri,
        PROV_STARTED_AT_TIME,
        &cell.started_at.to_rfc3339(),
        g,
    ));
    quads.push(literal_quad(
        &cell.iri,
        PROV_ENDED_AT_TIME,
        &cell.ended_at.to_rfc3339(),
        g,
    ));

    quads
}

fn named_quad_typed(s: &NamedNode, p: &str, o: NamedNode, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        NamedNodeRef::new_unchecked(p).into_owned(),
        o,
        g.clone(),
    )
}

fn literal_quad(s: &NamedNode, p: &str, v: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        NamedNodeRef::new_unchecked(p).into_owned(),
        Literal::new_simple_literal(v),
        g.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxigraph::model::Term;

    fn make_cell(
        name: &str,
        capability_iri: &str,
        usage: Option<WorkerResponseUsage>,
    ) -> CellSessionRecord {
        CellSessionRecord {
            iri: NamedNode::new_unchecked(format!(
                "urn:dec:cluster-session:add-judge-worker/FT-146/{name}"
            )),
            capability: NamedNode::new_unchecked(capability_iri),
            usage,
            status: CellStatus::Succeeded,
            started_at: Utc::now(),
            ended_at: Utc::now(),
        }
    }

    /// FT-146 TC: a Scaleway cell with usage produces the four token-
    /// breakdown quads + capability link + worker-reported usageSource.
    /// All cache fields zero — passes FT-057 SHACL for Scaleway.
    #[test]
    fn cell_quads_emit_token_breakdown_and_links_for_scaleway() {
        let cluster = NamedNode::new_unchecked("urn:dec:cluster-dispatch:add-judge-worker/FT-146");
        let cell = make_cell(
            "agent_loop",
            "https://decision-cli.dev/ns/capability/scaleway-coder/v1",
            Some(WorkerResponseUsage {
                input_tokens_base: 1234,
                input_tokens_cache_write: 0,
                input_tokens_cache_hit: 0,
                output_tokens: 567,
            }),
        );
        let g: GraphName = orchestration_graph().into_owned().into();
        let quads = cell_session_quads(&cluster, &cell, &g);

        // rdf:type → dec:Session.
        let has_type_session = quads.iter().any(|q| {
            q.subject == cell.iri.clone().into()
                && q.predicate.as_str() == RDF_TYPE
                && matches!(&q.object, Term::NamedNode(n)
                    if n.as_str() == "https://decision-cli.dev/ns#Session")
        });
        assert!(
            has_type_session,
            "cell session must carry rdf:type dec:Session"
        );

        // dec:capability link to the Scaleway capability.
        let has_capability = quads.iter().any(|q| {
            q.predicate.as_str() == "https://decision-cli.dev/ns#capability"
                && matches!(&q.object, Term::NamedNode(n)
                    if n.as_str().contains("scaleway-coder"))
        });
        assert!(has_capability, "cell session must link the capability");

        // Four token-breakdown predicates with the expected values.
        let token_pred_value = |pred: &str| -> Option<String> {
            quads.iter().find_map(|q| {
                if q.predicate.as_str() == pred {
                    if let Term::Literal(lit) = &q.object {
                        return Some(lit.value().to_string());
                    }
                }
                None
            })
        };
        assert_eq!(
            token_pred_value("https://decision-cli.dev/ns#input_tokens_base").as_deref(),
            Some("1234")
        );
        assert_eq!(
            token_pred_value("https://decision-cli.dev/ns#input_tokens_cache_write").as_deref(),
            Some("0")
        );
        assert_eq!(
            token_pred_value("https://decision-cli.dev/ns#input_tokens_cache_hit").as_deref(),
            Some("0")
        );
        assert_eq!(
            token_pred_value("https://decision-cli.dev/ns#output_tokens").as_deref(),
            Some("567")
        );

        // prov:wasInformedBy → parent cluster IRI.
        let has_parent_link = quads.iter().any(|q| {
            q.predicate.as_str() == PROV_WAS_INFORMED_BY
                && matches!(&q.object, Term::NamedNode(n) if n == &cluster)
        });
        assert!(
            has_parent_link,
            "cell session must reference the parent cluster activity"
        );

        // dec:usageSource = worker-reported.
        let usage_source = quads.iter().find_map(|q| {
            if q.predicate.as_str() == IRI_DEC_USAGE_SOURCE {
                if let Term::Literal(lit) = &q.object {
                    return Some(lit.value().to_string());
                }
            }
            None
        });
        assert_eq!(usage_source.as_deref(), Some("worker-reported"));
    }

    /// FT-146 TC: a mechanical cell (no LLM call, usage=None) writes
    /// `dec:cellStatus = "mechanical"`, `dec:usageSource = "unreported"`,
    /// and zero tokens. PROV-O coverage stays uniform across cell flavours.
    #[test]
    fn mechanical_cell_records_zero_tokens_and_unreported_source() {
        let cluster =
            NamedNode::new_unchecked("urn:dec:cluster-dispatch:add-cli-subcommand/FT-146");
        let mut cell = make_cell("iri_module_consts", "urn:dec:capability:mechanical", None);
        cell.status = CellStatus::Mechanical;
        let g: GraphName = orchestration_graph().into_owned().into();
        let quads = cell_session_quads(&cluster, &cell, &g);

        let status = quads.iter().find_map(|q| {
            if q.predicate.as_str() == IRI_DEC_CELL_STATUS {
                if let Term::Literal(lit) = &q.object {
                    return Some(lit.value().to_string());
                }
            }
            None
        });
        assert_eq!(status.as_deref(), Some("mechanical"));

        let usage_source = quads.iter().find_map(|q| {
            if q.predicate.as_str() == IRI_DEC_USAGE_SOURCE {
                if let Term::Literal(lit) = &q.object {
                    return Some(lit.value().to_string());
                }
            }
            None
        });
        assert_eq!(usage_source.as_deref(), Some("unreported"));

        // All four token predicates are zero.
        for pred in [
            "https://decision-cli.dev/ns#input_tokens_base",
            "https://decision-cli.dev/ns#input_tokens_cache_write",
            "https://decision-cli.dev/ns#input_tokens_cache_hit",
            "https://decision-cli.dev/ns#output_tokens",
        ] {
            let v = quads.iter().find_map(|q| {
                if q.predicate.as_str() == pred {
                    if let Term::Literal(lit) = &q.object {
                        return Some(lit.value().to_string());
                    }
                }
                None
            });
            assert_eq!(
                v.as_deref(),
                Some("0"),
                "mechanical cell predicate {pred} must be 0"
            );
        }
    }

    /// FT-146 TC: the parent cluster activity carries rdf:type
    /// `dec:ClusterDispatch` + `prov:Activity`, start + end times, and
    /// the outcome enum.
    #[test]
    fn cluster_activity_quads_carry_type_timing_and_outcome() {
        let cluster = NamedNode::new_unchecked("urn:dec:cluster-dispatch:add-judge-worker/FT-146");
        let started = Utc::now();
        let ended = started + chrono::Duration::seconds(42);
        let g: GraphName = orchestration_graph().into_owned().into();
        let quads = cluster_activity_quads(
            &cluster,
            "FT-146",
            "add-judge-worker",
            started,
            ended,
            ClusterOutcome::Succeeded,
            &g,
        );

        let types: Vec<String> = quads
            .iter()
            .filter(|q| q.predicate.as_str() == RDF_TYPE)
            .filter_map(|q| match &q.object {
                Term::NamedNode(n) => Some(n.as_str().to_string()),
                _ => None,
            })
            .collect();
        assert!(types.contains(&PROV_ACTIVITY.to_string()));
        assert!(types.contains(&IRI_DEC_CLUSTER_DISPATCH.to_string()));

        let outcome = quads.iter().find_map(|q| {
            if q.predicate.as_str() == IRI_DEC_CLUSTER_OUTCOME {
                if let Term::Literal(lit) = &q.object {
                    return Some(lit.value().to_string());
                }
            }
            None
        });
        assert_eq!(outcome.as_deref(), Some("succeeded"));

        let has_started = quads
            .iter()
            .any(|q| q.predicate.as_str() == PROV_STARTED_AT_TIME);
        let has_ended = quads
            .iter()
            .any(|q| q.predicate.as_str() == PROV_ENDED_AT_TIME);
        assert!(has_started && has_ended);
    }
}
