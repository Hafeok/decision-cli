//! FT-107 — load defect feedback for a `(feature, env)` pair so the
//! bundle assembler can hand the verify-graph-author worker the runtime
//! evidence its existing graph produced.
//!
//! Lookup strategy is deliberately simple: walk `.dec/verify/graph/` for
//! graphs that verify the target feature AND run in the target env,
//! collect every TC their steps provide evidence for, then ask the
//! feedback read API for `class = "defect"` artifacts whose
//! `dec:sourceArtifact` is one of those TCs and whose
//! `dec:lifecycleState = "produced"` and `dec:targetRole = "verifier"`.
//!
//! The function is best-effort: any I/O or parse failure surfaces as an
//! empty vec so the dispatch handler degrades to today's behaviour
//! rather than failing the request.

use std::collections::HashSet;
use std::path::Path;

use crate::core::feedback::read::list_by_class;
use crate::core::ontology::verification_graph::{from_turtle, VerificationGraph};

// `DefectFeedbackRecord` moved to `core::feedback::defect_record` in
// FT-108 so both this loader and the implementer-side loader share the
// same wire shape. Re-export here for backward compatibility with the
// FT-107 import path used by the bundle module + tests.
pub use crate::core::feedback::DefectFeedbackRecord;

/// Load every defect feedback whose addressing chain ties it to the
/// `(feature_short, env_short)` pair.
///
/// Best-effort: I/O / parse failures yield an empty vec. The handler
/// treats "no defect feedback" as "use today's matcher behaviour".
pub fn load_for(
    workdir: &Path,
    feature_short: &str,
    env_short: &str,
) -> Vec<DefectFeedbackRecord> {
    let store_path = crate::core::store::orchestration_dump_path(workdir);
    let Ok(store) = crate::core::store::load_store_from_dump(&store_path) else {
        return Vec::new();
    };

    let superseded = superseded_graph_shorts(&store);
    let graph_dir = workdir.join(".dec").join("verify").join("graph");
    let candidate_graphs =
        collect_candidate_graphs(&graph_dir, feature_short, env_short, &superseded);
    if candidate_graphs.is_empty() {
        return Vec::new();
    }
    let tc_iris = collect_evidence_tcs(&candidate_graphs);
    if tc_iris.is_empty() {
        return Vec::new();
    }

    let all_defects = list_by_class(&store, "defect").unwrap_or_default();
    let mut out: Vec<DefectFeedbackRecord> = Vec::new();
    for fb in all_defects {
        if fb.target_role != "verifier" {
            continue;
        }
        if fb.lifecycle_state != "produced" {
            continue;
        }
        let Some(source) = fb.source_artifact.as_ref() else {
            continue;
        };
        if !tc_iris.contains(source.as_str()) {
            continue;
        }
        // Skip defects whose source graph has been superseded; the
        // graph that emitted them is no longer authoritative, so the
        // verify-graph-author has nothing useful to do with them.
        let source_short = extract_graph_short_id(fb.source_session.as_str()).unwrap_or_default();
        if !source_short.is_empty() && superseded.contains(&source_short) {
            continue;
        }
        let graph_id = locate_graph_for_tc(&candidate_graphs, source.as_str());
        out.push(DefectFeedbackRecord {
            feedback_iri: fb.iri.as_str().to_string(),
            class: fb.class.clone(),
            severity: fb.severity.as_str().to_string(),
            evidence: fb.evidence.clone(),
            source_tc: source.as_str().to_string(),
            graph_id,
        });
    }
    // Deterministic order: sort by feedback IRI so the bundle hash is
    // stable across runs that see the same feedback set.
    out.sort_by(|a, b| a.feedback_iri.cmp(&b.feedback_iri));
    out
}

/// Set of graph short ids in the store with a `dec:supersededBy`
/// edge (i.e., retired graphs).
fn superseded_graph_shorts(
    store: &oxigraph::store::Store,
) -> std::collections::HashSet<String> {
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

/// Best-effort: pull the VG-NNN segment out of a session activity URI.
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

fn collect_candidate_graphs(
    graph_dir: &Path,
    feature_short: &str,
    env_short: &str,
    superseded: &std::collections::HashSet<String>,
) -> Vec<VerificationGraph> {
    let Ok(read) = std::fs::read_dir(graph_dir) else {
        return Vec::new();
    };
    let feature_iri_suffix = format!("/feature/{feature_short}");
    let env_iri_suffix = format!("/env/{env_short}");
    let mut out: Vec<VerificationGraph> = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ttl") {
            continue;
        }
        let Ok(graph) = from_turtle(&path) else {
            continue;
        };
        let verifies_match = graph
            .verifies
            .0
            .as_str()
            .ends_with(&feature_iri_suffix);
        let env_match = graph.environment.as_str().ends_with(&env_iri_suffix);
        if !(verifies_match && env_match) {
            continue;
        }
        // Filter out superseded graphs — operator (or eventually the
        // worker itself) marked the design as retired; don't bother
        // re-presenting its defects as if they still matter.
        let short = graph
            .id
            .as_str()
            .split('/')
            .last()
            .unwrap_or_default()
            .to_string();
        if superseded.contains(&short) {
            continue;
        }
        out.push(graph);
    }
    out
}

fn collect_evidence_tcs(graphs: &[VerificationGraph]) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for g in graphs {
        for step in &g.steps {
            for tc in &step.provides_evidence_for {
                out.insert(tc.as_str().to_string());
            }
        }
    }
    out
}

fn locate_graph_for_tc(graphs: &[VerificationGraph], tc_iri: &str) -> String {
    for g in graphs {
        for step in &g.steps {
            if step
                .provides_evidence_for
                .iter()
                .any(|tc| tc.as_str() == tc_iri)
            {
                let id = g.id.as_str();
                let short = id
                    .strip_prefix(crate::core::vocab::IRI_DEC_VERIFY_GRAPH_PREFIX)
                    .unwrap_or(id);
                return short.to_string();
            }
        }
    }
    String::new()
}
