//! FT-108 — load defect feedback targeted at the `implementer` role for
//! a feature's TCs. Mirror of [`crate::features::verify_graph_generate::defect_feedback`]
//! but with a simpler join: per-TC, since the implementer cares about
//! "which of my feature's tests fail" and doesn't filter by env.
//!
//! Best-effort: any I/O or parse failure yields an empty vec so the
//! dispatch handler degrades to today's behaviour rather than failing.

use std::path::Path;

use crate::core::feedback::read::list_by_class;
use crate::core::feedback::DefectFeedbackRecord;

/// Load every `produced`-state, `targetRole=implementer`, `class=defect`
/// feedback whose `dec:sourceArtifact` is one of `tc_iris`.
///
/// `tc_iris` should be the full IRI form (`https://decision-cli.dev/ns/tc/TC-NNN`);
/// callers typically derive them via `core::verify::coverage::feature_resolver::tc_iri_for`.
pub fn load_for_implementer(workdir: &Path, tc_iris: &[String]) -> Vec<DefectFeedbackRecord> {
    if tc_iris.is_empty() {
        return Vec::new();
    }
    let dump = crate::core::store::orchestration_dump_path(workdir);
    let Ok(store) = crate::core::store::load_store_from_dump(&dump) else {
        return Vec::new();
    };

    let superseded = superseded_graph_shorts(&store);
    let all_defects = list_by_class(&store, "defect").unwrap_or_default();
    let mut out: Vec<DefectFeedbackRecord> = Vec::new();
    for fb in all_defects {
        if fb.target_role != "implementer" {
            continue;
        }
        if fb.lifecycle_state != "produced" {
            continue;
        }
        let Some(source) = fb.source_artifact.as_ref() else {
            continue;
        };
        let source_str = source.as_str();
        if !tc_iris.iter().any(|t| t == source_str) {
            continue;
        }
        let graph_short = extract_graph_short_id(fb.source_session.as_str()).unwrap_or_default();
        // Skip defects whose source graph has been superseded — the
        // graph that emitted them is no longer authoritative, so the
        // worker doesn't need to chase them. The audit trail
        // (`dec loop show`) still preserves them.
        if !graph_short.is_empty() && superseded.contains(&graph_short) {
            continue;
        }
        out.push(DefectFeedbackRecord {
            feedback_iri: fb.iri.as_str().to_string(),
            class: fb.class.clone(),
            severity: fb.severity.as_str().to_string(),
            evidence: fb.evidence.clone(),
            source_tc: source_str.to_string(),
            // Best-effort: lift the VG short id out of the source-
            // session activity URI so the worker can read the graph
            // file (.dec/verify/graph/VG-NNN.ttl) to see the exact
            // failing command. The activity URI shape is
            // `…/verify-feature/FT-NNN/VG-NNN/ts-…` or
            // `…/verify-graph-run/VG-NNN/ts-…`. Empty when the URI
            // doesn't follow the convention — we don't promise.
            graph_id: graph_short,
        });
    }
    out.sort_by(|a, b| a.feedback_iri.cmp(&b.feedback_iri));
    out
}

/// Collect the set of superseded graph short ids (e.g. `VG-054`) by
/// querying the store. Cheap SPARQL, run once per dispatch.
fn superseded_graph_shorts(store: &oxigraph::store::Store) -> std::collections::HashSet<String> {
    use oxigraph::sparql::QueryResults;
    let q = r#"PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?graph WHERE { GRAPH ?g { ?graph dec:supersededBy ?_succ . } }"#;
    let mut out = std::collections::HashSet::new();
    let Ok(QueryResults::Solutions(sols)) = store.query(q) else {
        return out;
    };
    for sol in sols.flatten() {
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("graph") {
            for segment in n.as_str().split('/') {
                if segment.starts_with("VG-")
                    && segment.len() > 3
                    && segment[3..].chars().all(|c| c.is_ascii_digit())
                {
                    out.insert(segment.to_string());
                    break;
                }
            }
        }
    }
    out
}

/// Best-effort extractor: pull the `VG-NNN` segment out of a
/// source-session activity URI. Returns `None` when the URI doesn't
/// contain a `/VG-NNN/` segment.
fn extract_graph_short_id(session_uri: &str) -> Option<String> {
    for segment in session_uri.split('/') {
        if segment.starts_with("VG-")
            && segment.len() > 3
            && segment[3..].chars().all(|c| c.is_ascii_digit())
        {
            return Some(segment.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_graph_short_id_pulls_vg_from_verify_feature_uri() {
        let uri = "https://decision-cli.dev/ns/activity/verify-feature/FT-012/VG-054/ts-1779954488359098791";
        assert_eq!(extract_graph_short_id(uri), Some("VG-054".to_string()));
    }

    #[test]
    fn extract_graph_short_id_pulls_vg_from_verify_graph_run_uri() {
        let uri =
            "https://decision-cli.dev/ns/activity/verify-graph-run/VG-097/ts-1779887801534721139";
        assert_eq!(extract_graph_short_id(uri), Some("VG-097".to_string()));
    }

    #[test]
    fn extract_graph_short_id_returns_none_when_absent() {
        let uri = "https://decision-cli.dev/ns/session/abc-123";
        assert_eq!(extract_graph_short_id(uri), None);
    }
}
