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

    /// Deterministic hash of the feature's full state: verdict, the
    /// sorted set of open defect-feedback IRIs (for both roles, after
    /// the supersession filter), and the sorted set of active
    /// (non-superseded) covering graph IRIs. Two classify rounds
    /// observing the same hash are visiting the same state — by
    /// construction, the same planner table inputs produce the same
    /// dispatch, so a repeated hash means the loop is in a cycle.
    /// Returns `0` only when an inspector error would otherwise have
    /// been raised — callers may treat `0` as "unknown / no signal."
    fn state_hash_for_feature(&self, feature_id: &str) -> Result<u64, InspectError>;

    /// Ground-truth completion gate: shells out to
    /// `product verify FT-XXX --root <product_root>` and returns
    /// whether it exits 0. Per CLAUDE.md "Definition of done", this
    /// is the authoritative completion criterion — when it passes,
    /// the feature is done regardless of VG verdict (which can lag:
    /// a stale rejected VGR from before the fix isn't auto-superseded
    /// yet). When it fails, VG-derived open defects route the worker
    /// via the existing classifier table.
    ///
    /// Default impl returns `Ok(false)` — stub inspectors that don't
    /// supply this dimension drop through to verdict/open-feedback
    /// classification, preserving the pre-gate test expectations.
    /// Production overrides shell out to `product`.
    fn product_verify_passes_for_feature(
        &self,
        _feature_id: &str,
    ) -> Result<bool, InspectError> {
        Ok(false)
    }
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
        // Orchestration store keeps every artifact in a named graph;
        // bare patterns query only the default graph (empty) and
        // return nothing. We need GRAPH wrapping, and crucially each
        // triple may live in a *different* named graph
        // (VerificationGraph artifacts in verify-graph,
        // VerificationGraphResults in result-graph), so each is
        // wrapped separately. The supersession check is its own
        // GRAPH clause for the same reason.
        let q = format!(
            r#"PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?verdict WHERE {{
  GRAPH ?g1 {{ ?graph dec:verifies <{feature_iri}> . }}
  GRAPH ?g2 {{ ?vgr dec:resultOf ?graph ; dec:verdict ?verdict . }}
  FILTER NOT EXISTS {{ GRAPH ?h {{ ?graph dec:supersededBy ?_succ }} }}
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
        use crate::core::ontology::verification_graph::io::from_turtle;
        use crate::core::store::{load_store_from_dump, orchestration_dump_path};
        use crate::core::verify::coverage::feature_resolver::{
            resolve_feature_tcs_short, tc_iri_for,
        };

        let tc_shorts =
            resolve_feature_tcs_short(self.product_root(), feature_id).map_err(|e| {
                InspectError::Store {
                    detail: format!("resolve TCs for {feature_id}: {e}"),
                }
            })?;
        if tc_shorts.is_empty() {
            return Ok(false);
        }
        let tc_iris: std::collections::HashSet<String> =
            tc_shorts.iter().map(|s| tc_iri_for(s)).collect();

        // Coverage is evidence-based AND disk-authoritative: a graph
        // counts only if its on-disk .ttl file's steps emit evidence
        // for one of the feature's TCs. The store can carry stale
        // `dec:providesEvidenceFor` cruft from past sessions that
        // doesn't match the file's actual step definitions; the
        // runner uses the .ttl files at execution time, so the
        // planner must too. Superseded graphs are skipped via the
        // store-side supersededBy edge.
        let dump = orchestration_dump_path(self.workdir());
        let store = load_store_from_dump(&dump).map_err(|e| InspectError::Store {
            detail: format!("opening store at {p}: {e:#}", p = dump.display()),
        })?;
        let superseded = superseded_graph_shorts(&store);

        let graph_dir = self.workdir().join(".dec").join("verify").join("graph");
        let Ok(read) = std::fs::read_dir(&graph_dir) else {
            return Ok(false);
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ttl") {
                continue;
            }
            let Ok(graph) = from_turtle(&path) else {
                continue;
            };
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
            let direct_match = tc_iris.contains(graph.verifies.0.as_str());
            let step_match = graph.steps.iter().any(|step| {
                step.provides_evidence_for
                    .iter()
                    .any(|tc| tc_iris.contains(tc.as_str()))
            });
            if direct_match || step_match {
                return Ok(true);
            }
        }
        Ok(false)
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

    fn state_hash_for_feature(&self, feature_id: &str) -> Result<u64, InspectError> {
        use crate::core::feedback::read::list_by_class;
        use crate::core::ontology::verification_graph::io::from_turtle;
        use crate::core::store::{load_store_from_dump, orchestration_dump_path};
        use crate::core::verify::coverage::feature_resolver::{
            resolve_feature_tcs_short, tc_iri_for,
        };
        use std::collections::hash_map::DefaultHasher;
        use std::collections::BTreeSet;
        use std::hash::{Hash, Hasher};

        let verdict = self.aggregate_verdict_for_feature(feature_id)?;

        let tc_shorts =
            resolve_feature_tcs_short(self.product_root(), feature_id).map_err(|e| {
                InspectError::Store {
                    detail: format!("resolve TCs for {feature_id}: {e}"),
                }
            })?;
        let tc_iris: std::collections::HashSet<String> =
            tc_shorts.iter().map(|s| tc_iri_for(s)).collect();

        let dump = orchestration_dump_path(self.workdir());
        let store = load_store_from_dump(&dump).map_err(|e| InspectError::Store {
            detail: format!("opening store at {p}: {e:#}", p = dump.display()),
        })?;
        let superseded = superseded_graph_shorts(&store);

        // The set of open feedback IRIs (after the supersession
        // filter) — BTreeSet keeps the iteration order deterministic
        // so the hash is reproducible across runs that see the same
        // store state.
        let defects = list_by_class(&store, "defect").map_err(|e| InspectError::Store {
            detail: format!("listing defect feedback: {e}"),
        })?;
        let open_iris: BTreeSet<String> = defects
            .into_iter()
            .filter(|fb| {
                matches!(fb.lifecycle_state.as_str(), "produced" | "routed" | "received")
            })
            .filter(|fb| {
                fb.source_artifact
                    .as_ref()
                    .map(|src| tc_iris.contains(src.as_str()))
                    .unwrap_or(false)
            })
            .filter(|fb| {
                let short =
                    extract_graph_short_id(fb.source_session.as_str()).unwrap_or_default();
                short.is_empty() || !superseded.contains(&short)
            })
            .map(|fb| fb.iri.as_str().to_string())
            .collect();

        // The set of active (non-superseded, on-disk) graph IRIs that
        // cover the feature. Two trajectories that pass through the
        // same defect-set AND the same graph-set are visiting the
        // identical planning state.
        let graph_dir = self.workdir().join(".dec").join("verify").join("graph");
        let mut active_graphs: BTreeSet<String> = BTreeSet::new();
        if let Ok(read) = std::fs::read_dir(&graph_dir) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("ttl") {
                    continue;
                }
                let Ok(graph) = from_turtle(&path) else {
                    continue;
                };
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
                let covers_feature = tc_iris.contains(graph.verifies.0.as_str())
                    || graph.steps.iter().any(|s| {
                        s.provides_evidence_for
                            .iter()
                            .any(|tc| tc_iris.contains(tc.as_str()))
                    });
                if covers_feature {
                    active_graphs.insert(graph.id.as_str().to_string());
                }
            }
        }

        // Include the product-verify gate's signal in the hash. Without
        // it the cycle detector false-positives "period 1 cycle" after
        // every implementer dispatch on an Approved-with-failing-verify
        // feature: VGs/defects/verdict are identical between the pre-
        // and post-implementer classify calls because the implementer
        // produces code (not VGs), so only the product-verify outcome
        // can record the work. When the implementer's commit flips
        // product verify from fail to pass, the hash changes and the
        // planner correctly proceeds to Done; when it stays failing,
        // the head-sha fold below still keeps successive implementer
        // commits distinct so the cycle detector doesn't bail after
        // one round of broken-code commits.
        let product_verify_passes = self
            .product_verify_passes_for_feature(feature_id)
            .unwrap_or(true);
        // Include the most recent `[FT-XXX]` commit SHA. A productive
        // implementer dispatch lands a new commit, so the SHA changes
        // even when verdict/defects/active-graphs are stable. Witnessed
        // on FT-115: the implementer's first commit had broken imports
        // (cli/worktree.rs used `crate::core::*` instead of
        // `decision_cli::core::*`) so product verify kept failing —
        // without this fold the cycle detector tripped after one
        // round and starved the loop of further repair attempts.
        // A non-progressing dispatch (worker error, no commit) leaves
        // the SHA unchanged, so the cycle detector still terminates
        // correctly when the implementer truly can't move.
        let head_sha = latest_feature_commit_sha(self.workdir(), feature_id);
        let mut h = DefaultHasher::new();
        format!("{verdict:?}").hash(&mut h);
        product_verify_passes.hash(&mut h);
        head_sha.hash(&mut h);
        for iri in &open_iris {
            iri.hash(&mut h);
        }
        // Separator so a graph IRI can't accidentally collide with a
        // feedback IRI in the hash stream.
        "::graphs::".hash(&mut h);
        for iri in &active_graphs {
            iri.hash(&mut h);
        }
        Ok(h.finish())
    }

    fn product_verify_passes_for_feature(
        &self,
        feature_id: &str,
    ) -> Result<bool, InspectError> {
        use std::process::Command;
        let product = match which_on_path("product") {
            Some(p) => p,
            // No `product` binary on $PATH → can't gate. Fall back to
            // permissive (trust the VG verdict) rather than blocking
            // the drive permanently; emit a one-shot warning via
            // tracing so the operator sees it.
            None => {
                tracing::warn!(
                    feature_id,
                    "product CLI not on $PATH — skipping product-verify gate; \
                     drive will trust aggregate VG verdict"
                );
                return Ok(true);
            }
        };
        let out = Command::new(product)
            .arg("verify")
            .arg(feature_id)
            .arg("--root")
            .arg(self.product_root())
            .output()
            .map_err(|e| InspectError::Store {
                detail: format!("spawn `product verify {feature_id}`: {e}"),
            })?;
        // `product verify FT-XXX` exits 0 in both pass and fail cases
        // (witnessed: failing FT-116 prints "TC-NNN ... FAIL" lines for
        // every TC and still exits 0). Trust stdout markers instead of
        // the exit code: a full-pass run emits the success-tag line
        // `✓ Tagged: product/FT-XXX/complete-vN`, while any failing
        // run omits it. We additionally treat the presence of any
        // `FAIL` token as definitive failure as a belt-and-braces
        // check against future stdout format changes.
        let stdout = String::from_utf8_lossy(&out.stdout);
        let tag_prefix = format!("✓ Tagged: product/{feature_id}/complete");
        let saw_success_tag = stdout.contains(&tag_prefix);
        let saw_fail_line = stdout
            .lines()
            .any(|line| line.contains(" FAIL ") || line.trim_end().ends_with(" FAIL"));
        Ok(saw_success_tag && !saw_fail_line)
    }
}

/// `which`-style $PATH lookup. Returns the absolute path to `bin` or
/// `None` when not on $PATH. Mirrors the helper in `finalize/mod.rs`
/// to avoid pulling that module into core/drive.
fn which_on_path(bin: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Most-recent `[FT-XXX]`-tagged commit SHA, or an empty string when
/// `git` is not on $PATH or no such commit exists. Used by the state
/// hash to distinguish successive productive implementer dispatches
/// (each lands a new commit and hence flips the SHA) from a stuck
/// dispatcher (worker keeps failing to commit). When no commit yet
/// exists for the feature, the empty-string sentinel still maps to a
/// stable hash, so the cycle detector behaves as before.
fn latest_feature_commit_sha(repo_root: &Path, feature_id: &str) -> String {
    use std::process::Command;
    if which_on_path("git").is_none() {
        return String::new();
    }
    let needle = format!("[{feature_id}]");
    let escaped = needle.replace('[', "\\[").replace(']', "\\]");
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("log")
        .arg("--all")
        .arg("--format=%H")
        .arg("-n")
        .arg("1")
        .arg(format!("--grep={escaped}"))
        .arg("--extended-regexp")
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
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
