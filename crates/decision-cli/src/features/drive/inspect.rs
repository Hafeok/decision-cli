//! State-inspection trait used by planners.
//!
//! Planners depend on a small trait surface rather than on the
//! `PlanContext` directly, so unit tests can supply a stub that pins
//! the (verdict, open-feedback counts) state without seeding the live
//! orchestration store. Production code wires the trait against the
//! real readers (verify-feature aggregator, FT-108 defect loader).

use std::path::Path;

use crate::core::drive::PlanContext;

/// Aggregate verdict for a feature, as reported by `dec verify
/// feature FT-XXX` across every covering graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureVerdict {
    /// Every covering graph approved.
    Approved,
    /// At least one covering graph emitted `rejected` (evidence
    /// regression).
    Rejected,
    /// At least one covering graph emitted `amendment-required` (graph
    /// setup-failure).
    AmendmentRequired,
    /// No verify run on record for this feature.
    NeverRun,
}

/// Trait planners depend on. Production impl lives in
/// [`ProductionInspector`]; tests build their own.
pub trait GraphInspector {
    /// Aggregate verify verdict for the named feature.
    fn aggregate_verdict_for_feature(
        &self,
        feature_id: &str,
    ) -> Result<FeatureVerdict, InspectError>;

    /// Count of open (`produced | routed | received`) defect feedback
    /// entries targeting `role_id` whose source artifact is one of the
    /// feature's TCs.
    fn open_defect_feedback_count(
        &self,
        feature_id: &str,
        role_id: &str,
    ) -> Result<usize, InspectError>;

    /// `true` if at least one VerificationGraph exists for the named
    /// feature in the current store. Used to distinguish "no verify
    /// run on record because the graphs haven't been run" from "no
    /// verify run on record because no graphs even exist yet" — the
    /// former wants a verifier dispatch, the latter wants a
    /// verify-graph-author dispatch to author one first.
    fn graphs_exist_for_feature(&self, feature_id: &str) -> Result<bool, InspectError>;
}

/// Inspector errors. Kept generic so planners can propagate
/// without knowing the underlying read API.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InspectError {
    /// Underlying store read failed.
    #[error("inspector: {detail}")]
    Store {
        /// Human-readable detail.
        detail: String,
    },
}

/// Map a verdict string to a `FeatureVerdict`.
fn parse_verdict(s: &str) -> FeatureVerdict {
    match s {
        "approved" => FeatureVerdict::Approved,
        "amendment-required" => FeatureVerdict::AmendmentRequired,
        "rejected" => FeatureVerdict::Rejected,
        _ => FeatureVerdict::NeverRun,
    }
}

/// Reduce two per-graph verdicts to the worse one, matching the
/// FT-097 aggregate rule (Rejected > AmendmentRequired > Approved).
fn combine_worst(a: FeatureVerdict, b: FeatureVerdict) -> FeatureVerdict {
    fn rank(v: FeatureVerdict) -> u8 {
        match v {
            FeatureVerdict::Approved => 0,
            FeatureVerdict::NeverRun => 1,
            FeatureVerdict::AmendmentRequired => 2,
            FeatureVerdict::Rejected => 3,
        }
    }
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

/// Production inspector that reads against the real orchestration
/// store via the existing FT-099 / FT-108 surfaces. Constructed once
/// per driver invocation; cheap to hold across iterations.
pub struct ProductionInspector<'a> {
    ctx: &'a PlanContext,
}

impl<'a> ProductionInspector<'a> {
    /// Wire against a planning context.
    #[must_use]
    pub fn new(ctx: &'a PlanContext) -> Self {
        Self { ctx }
    }

    fn workdir(&self) -> &Path {
        &self.ctx.workdir
    }

    fn product_root(&self) -> &Path {
        &self.ctx.product_root
    }
}

impl<'a> GraphInspector for ProductionInspector<'a> {
    fn aggregate_verdict_for_feature(
        &self,
        feature_id: &str,
    ) -> Result<FeatureVerdict, InspectError> {
        // Cheap read: aggregate verdict from already-persisted VGRs.
        // Avoids re-running graphs on every planner iteration; the
        // planner only needs the *current* state, not a fresh verify
        // pass.
        use crate::core::store::{load_store_from_dump, orchestration_dump_path};
        use oxigraph::sparql::QueryResults;

        let feature_iri = format!("https://decision-cli.dev/ns/feature/{feature_id}");
        let dump = orchestration_dump_path(self.workdir());
        let store = load_store_from_dump(&dump).map_err(|e| InspectError::Store {
            detail: format!("opening store at {p}: {e:#}", p = dump.display()),
        })?;
        let q = format!(
            r#"PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?verdict WHERE {{
  ?graph dec:verifies <{feature_iri}> .
  ?vgr dec:resultOf ?graph ;
       dec:verdict ?verdict .
  FILTER NOT EXISTS {{ ?graph dec:supersededBy ?_succ }}
}}"#
        );
        let solutions = match store.query(&q) {
            Ok(QueryResults::Solutions(s)) => s,
            Ok(_) => {
                return Err(InspectError::Store {
                    detail: "verdict query returned non-solution shape".to_string(),
                });
            }
            Err(e) => {
                return Err(InspectError::Store {
                    detail: format!("verdict query failed: {e}"),
                });
            }
        };
        let mut any = false;
        let mut worst = FeatureVerdict::Approved;
        for sol in solutions {
            let sol = sol.map_err(|e| InspectError::Store {
                detail: format!("verdict solution iteration: {e}"),
            })?;
            let verdict_str = sol
                .get("verdict")
                .and_then(|t| match t {
                    oxigraph::model::Term::Literal(l) => Some(l.value().to_string()),
                    _ => None,
                })
                .unwrap_or_default();
            any = true;
            worst = combine_worst(worst, parse_verdict(&verdict_str));
        }
        if !any {
            return Ok(FeatureVerdict::NeverRun);
        }
        Ok(worst)
    }

    fn graphs_exist_for_feature(&self, feature_id: &str) -> Result<bool, InspectError> {
        use crate::core::store::{load_store_from_dump, orchestration_dump_path};
        use oxigraph::sparql::QueryResults;

        let feature_iri = format!("https://decision-cli.dev/ns/feature/{feature_id}");
        let dump = orchestration_dump_path(self.workdir());
        let store = load_store_from_dump(&dump).map_err(|e| InspectError::Store {
            detail: format!("opening store at {p}: {e:#}", p = dump.display()),
        })?;
        let q = format!(
            r#"PREFIX dec: <https://decision-cli.dev/ns#>
ASK WHERE {{ GRAPH ?g {{
  ?graph dec:verifies <{feature_iri}> .
  FILTER NOT EXISTS {{ ?graph dec:supersededBy ?_succ }}
}} }}"#
        );
        match store.query(&q) {
            Ok(QueryResults::Boolean(b)) => Ok(b),
            Ok(_) => Err(InspectError::Store {
                detail: "graphs-exist query returned non-boolean shape".to_string(),
            }),
            Err(e) => Err(InspectError::Store {
                detail: format!("graphs-exist query failed: {e}"),
            }),
        }
    }

    fn open_defect_feedback_count(
        &self,
        feature_id: &str,
        role_id: &str,
    ) -> Result<usize, InspectError> {
        use crate::core::feedback::read::list_by_class;
        use crate::core::store::{load_store_from_dump, orchestration_dump_path};
        use crate::core::verify::coverage::feature_resolver::{
            resolve_feature_tcs_short, tc_iri_for,
        };

        let tc_shorts = resolve_feature_tcs_short(self.product_root(), feature_id).map_err(|e| {
            InspectError::Store {
                detail: format!("resolve TCs for {feature_id}: {e}"),
            }
        })?;
        if tc_shorts.is_empty() {
            return Ok(0);
        }
        let tc_iris: std::collections::HashSet<String> =
            tc_shorts.iter().map(|s| tc_iri_for(s)).collect();

        let dump = orchestration_dump_path(self.workdir());
        let store = load_store_from_dump(&dump).map_err(|e| InspectError::Store {
            detail: format!("opening store at {p}: {e:#}", p = dump.display()),
        })?;
        let superseded = superseded_graph_shorts(&store);
        let defects = list_by_class(&store, "defect").map_err(|e| InspectError::Store {
            detail: format!("listing defect feedback: {e}"),
        })?;
        let count = defects
            .into_iter()
            .filter(|fb| fb.target_role == role_id)
            .filter(|fb| {
                matches!(fb.lifecycle_state.as_str(), "produced" | "routed" | "received")
            })
            .filter(|fb| {
                fb.source_artifact
                    .as_ref()
                    .map(|src| tc_iris.contains(src.as_str()))
                    .unwrap_or(false)
            })
            // Skip defects emitted by superseded graphs — the workers'
            // bundle loaders hide them too, so counting them here would
            // make the planner dispatch workers that see nothing to do.
            .filter(|fb| {
                let short =
                    extract_graph_short_id(fb.source_session.as_str()).unwrap_or_default();
                short.is_empty() || !superseded.contains(&short)
            })
            .count();
        Ok(count)
    }
}

/// Set of graph short ids (`VG-NNN`) in the store with a
/// `dec:supersededBy` edge.
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
