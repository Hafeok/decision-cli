//! Orchestrates the (find → retract) flow for orphan-defect retraction.

use std::path::Path;
use std::sync::Arc;

use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::StreamWriter;

use super::query::{
    find_orphan_defects_all, find_orphan_defects_for_feature, find_orphan_defects_for_graph,
    OrphanDefect,
};
use super::transition::retract_orphan;

/// Scope for an orphan-retraction sweep.
#[derive(Debug, Clone)]
pub enum RetractScope {
    /// One graph by id (`VG-NNN`).
    Graph(String),
    /// One feature by id (`FT-NNN`); all linked graphs are swept.
    Feature(String),
    /// Every graph in the store.
    All,
}

/// Per-graph retraction tally returned by the sweep.
#[derive(Debug, Clone)]
pub struct GraphTally {
    /// Short id of the VG, e.g. `VG-088`.
    pub vg_id: String,
    /// Count of orphan feedbacks retracted (or that would be retracted in
    /// dry-run mode).
    pub retracted: usize,
    /// Per-candidate detail for dry-run / verbose output.
    pub candidates: Vec<OrphanDefect>,
}

/// Aggregate result returned by [`retract_orphans_for_graph`],
/// [`retract_orphans_for_feature`], and [`retract_orphans_all`].
#[derive(Debug, Clone)]
pub struct RetractSummary {
    /// Per-graph rows.
    pub per_graph: Vec<GraphTally>,
    /// Sum across `per_graph`.
    pub total: usize,
}

/// Retract orphan defects for one VG. Returns the tally (always exactly
/// one row in `per_graph`).
pub fn retract_orphans_for_graph(
    workdir: &Path,
    vg_id: &str,
    dry_run: bool,
) -> Result<RetractSummary, String> {
    run_scope(workdir, &RetractScope::Graph(vg_id.to_string()), dry_run)
}

/// Retract orphan defects across all VGs linked to a feature.
pub fn retract_orphans_for_feature(
    workdir: &Path,
    feature_id: &str,
    dry_run: bool,
) -> Result<RetractSummary, String> {
    run_scope(workdir, &RetractScope::Feature(feature_id.to_string()), dry_run)
}

/// Retract orphan defects across every VG in the store.
pub fn retract_orphans_all(workdir: &Path, dry_run: bool) -> Result<RetractSummary, String> {
    run_scope(workdir, &RetractScope::All, dry_run)
}

fn run_scope(
    workdir: &Path,
    scope: &RetractScope,
    dry_run: bool,
) -> Result<RetractSummary, String> {
    let dump_path = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump_path).map_err(|e| format!("loading store: {e:#}"))?;
    let store = Arc::new(store);

    let candidates = find_candidates(&store, scope)?;
    let by_graph = group_by_graph(candidates);

    if dry_run {
        let per_graph: Vec<GraphTally> = by_graph
            .into_iter()
            .map(|(vg_iri, defects)| GraphTally {
                vg_id: extract_short_id(&vg_iri),
                retracted: defects.len(),
                candidates: defects,
            })
            .collect();
        let total = per_graph.iter().map(|g| g.retracted).sum();
        return Ok(RetractSummary { per_graph, total });
    }

    let scope_obj = crate::core::scope::ActiveScope::load(workdir)
        .map_err(|e| format!("loading scope: {e}"))?;
    let stream_iri = NamedNode::new(&scope_obj.stream_iri)
        .map_err(|e| format!("invalid stream IRI: {e}"))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .map_err(|e| format!("opening writer: {e:#}"))?;

    let mut per_graph: Vec<GraphTally> = Vec::new();
    let mut total = 0usize;

    for (vg_iri, defects) in by_graph {
        let mut retracted = 0usize;
        for defect in &defects {
            // retract_orphan returns false for terminal states (idempotent skip).
            // Errors here surface as a partial-failure with the tally so far.
            let did = retract_orphan(
                &store,
                &writer,
                &defect.feedback_iri,
                &defect.session_iri,
                &defect.vg_iri,
                &defect.tc_iri,
            )
            .map_err(|e| format!("retracting {}: {e}", defect.feedback_iri))?;
            if did {
                retracted += 1;
            }
        }
        total += retracted;
        per_graph.push(GraphTally {
            vg_id: extract_short_id(&vg_iri),
            retracted,
            candidates: defects,
        });
    }

    persist_store(&store, &dump_path).map_err(|e| format!("persisting store: {e:#}"))?;

    Ok(RetractSummary { per_graph, total })
}

fn find_candidates(
    store: &Store,
    scope: &RetractScope,
) -> Result<Vec<OrphanDefect>, String> {
    match scope {
        RetractScope::Graph(vg_id) => {
            let vg_iri = if vg_id.starts_with("https://") {
                vg_id.clone()
            } else {
                format!("https://decision-cli.dev/ns/verify/graph/{vg_id}")
            };
            find_orphan_defects_for_graph(store, &vg_iri)
        }
        RetractScope::Feature(ft_id) => {
            let ft_iri = if ft_id.starts_with("https://") {
                ft_id.clone()
            } else {
                format!("https://decision-cli.dev/ns/feature/{ft_id}")
            };
            find_orphan_defects_for_feature(store, &ft_iri)
        }
        RetractScope::All => find_orphan_defects_all(store),
    }
}

fn group_by_graph(candidates: Vec<OrphanDefect>) -> Vec<(String, Vec<OrphanDefect>)> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, Vec<OrphanDefect>> = BTreeMap::new();
    for d in candidates {
        map.entry(d.vg_iri.clone()).or_default().push(d);
    }
    map.into_iter().collect()
}

fn extract_short_id(iri: &str) -> String {
    iri.rsplit('/').next().unwrap_or(iri).to_string()
}
