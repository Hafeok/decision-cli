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
        out.push(DefectFeedbackRecord {
            feedback_iri: fb.iri.as_str().to_string(),
            class: fb.class.clone(),
            severity: fb.severity.as_str().to_string(),
            evidence: fb.evidence.clone(),
            source_tc: source_str.to_string(),
            // The implementer loader joins by TC alone; we deliberately
            // leave graph_id empty (verify-graph-author is the consumer
            // that cares which graph).
            graph_id: String::new(),
        });
    }
    out.sort_by(|a, b| a.feedback_iri.cmp(&b.feedback_iri));
    out
}
