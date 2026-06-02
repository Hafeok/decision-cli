//! Production overrides for the FT-119 (Definition-of-Ready) inspector
//! dimensions. Kept out of `inspect.rs` to honour ADR-013's
//! file-length cap — the trait + production base already weigh in well
//! over the warn band.
//!
//! Each function below mirrors one [`GraphInspector`] trait method
//! that defaults to "ready" in the trait. The trait impl on
//! `ProductionInspector` delegates to these helpers so it can keep
//! the trait surface short and unit-testable in isolation.
//!
//! [`GraphInspector`]: super::inspect::GraphInspector

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use oxigraph::sparql::QueryResults;
use serde::Deserialize;

use super::inspect::{
    CoveringGraphState, InspectError, PreflightGap, PreflightStatus, SpecCompleteness,
    TcsLinkedState,
};

/// Required top-level H2 sections in a feature spec body. Mirrors
/// product-cli's [`default_required_sections`] so DoR and product's
/// W030 completeness check agree on what "complete" means without
/// re-reading `product.toml`.
///
/// [`default_required_sections`]: https://github.com/Hafeok/product-cli/blob/main/src/config_features.rs
const REQUIRED_H2_SECTIONS: &[&str] = &[
    "Description",
    "Functional Specification",
    "Out of scope",
];

// ---------------------------------------------------------------------
// Preflight — shells out to `product preflight FT-XXX --format json`.
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PreflightJson {
    cross_cutting_gaps: Vec<PreflightGapJson>,
    #[serde(default)]
    domain_gaps: Vec<DomainGapJson>,
}

#[derive(Debug, Deserialize)]
struct PreflightGapJson {
    adr_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct DomainGapJson {
    domain: String,
    status: String,
}

/// Run `product preflight FT-XXX --root <product_root> --format json`
/// and translate gaps into [`PreflightStatus`]. Gaps are sorted
/// lexicographically so the planner's Stuck reason is byte-deterministic
/// (TC-255 boundary requirement).
///
/// When `product` is not on `$PATH` we return `Clean` and let the
/// planner trust the rest of the inspector — same fail-permissive
/// stance the existing `product_verify_passes_for_feature` takes when
/// the CLI is missing.
pub(super) fn preflight_status(
    product_root: &Path,
    feature_id: &str,
) -> Result<PreflightStatus, InspectError> {
    let Some(product_bin) = which_on_path("product") else {
        tracing::warn!(
            feature_id,
            "product CLI not on $PATH — DoR preflight check returning Clean"
        );
        return Ok(PreflightStatus::Clean);
    };
    let output = std::process::Command::new(product_bin)
        .arg("preflight")
        .arg(feature_id)
        .arg("--root")
        .arg(product_root)
        .arg("--format")
        .arg("json")
        .output()
        .map_err(|e| InspectError::Store {
            detail: format!("spawn `product preflight {feature_id}`: {e}"),
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: PreflightJson =
        serde_json::from_str(&stdout).map_err(|e| InspectError::Store {
            detail: format!("parse preflight JSON for {feature_id}: {e}"),
        })?;
    let mut gaps: Vec<PreflightGap> = Vec::new();
    for g in parsed.cross_cutting_gaps {
        if g.status == "gap" {
            gaps.push(PreflightGap::UnacknowledgedAdr(g.adr_id));
        }
    }
    for g in parsed.domain_gaps {
        if g.status == "gap" {
            gaps.push(PreflightGap::UncoveredDomain(g.domain));
        }
    }
    if gaps.is_empty() {
        return Ok(PreflightStatus::Clean);
    }
    sort_gaps(&mut gaps);
    Ok(PreflightStatus::Warnings { gaps })
}

fn sort_gaps(gaps: &mut [PreflightGap]) {
    gaps.sort_by(|a, b| variant_key(a).cmp(&variant_key(b)));
}

fn variant_key(g: &PreflightGap) -> (u8, &str) {
    match g {
        PreflightGap::UnacknowledgedAdr(id) => (0, id.as_str()),
        PreflightGap::UncoveredDomain(d) => (1, d.as_str()),
    }
}

// ---------------------------------------------------------------------
// Dependency statuses — pure markdown read of feature + each dep.
// ---------------------------------------------------------------------

/// Read the feature's `depends-on` list and resolve each dep's
/// `status` field. Returns the pairs in spec-declaration order so the
/// planner's "first unfinished dep" check is stable. Missing dep
/// files surface as `status = "missing"` rather than an inspector
/// error — the planner classifies that as Stuck which is the
/// operator-visible signal.
pub(super) fn dependency_statuses(
    product_root: &Path,
    feature_id: &str,
) -> Result<Vec<(String, String)>, InspectError> {
    let feature_path = find_feature_file(product_root, feature_id).map_err(store)?;
    let body = fs::read_to_string(&feature_path).map_err(|e| InspectError::Store {
        detail: format!("read {p}: {e}", p = feature_path.display()),
    })?;
    let frontmatter = extract_frontmatter(&body).unwrap_or("");
    let deps = extract_yaml_list(frontmatter, "depends-on");
    let mut out: Vec<(String, String)> = Vec::with_capacity(deps.len());
    for dep_id in deps {
        let status = match find_feature_file(product_root, &dep_id) {
            Ok(dep_path) => read_status_field(&dep_path).unwrap_or_else(|_| "unknown".to_string()),
            Err(_) => "missing".to_string(),
        };
        out.push((dep_id, status));
    }
    Ok(out)
}

fn read_status_field(path: &Path) -> Result<String, InspectError> {
    let body = fs::read_to_string(path).map_err(|e| InspectError::Store {
        detail: format!("read {p}: {e}", p = path.display()),
    })?;
    let frontmatter = extract_frontmatter(&body).unwrap_or("");
    for line in frontmatter.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("status:") {
            return Ok(rest.trim().trim_matches('"').to_string());
        }
    }
    Ok("unknown".to_string())
}

// ---------------------------------------------------------------------
// Spec completeness — H2 header presence check.
// ---------------------------------------------------------------------

/// Walk the feature body (everything after the frontmatter) for the
/// product-cli W030 required H2 sections. Returns the first missing
/// heading in declaration order so the planner's Stuck reason cites
/// the exact section the operator should add (TC-255).
pub(super) fn feature_spec_completeness(
    product_root: &Path,
    feature_id: &str,
) -> Result<SpecCompleteness, InspectError> {
    let feature_path = find_feature_file(product_root, feature_id).map_err(store)?;
    let body = fs::read_to_string(&feature_path).map_err(|e| InspectError::Store {
        detail: format!("read {p}: {e}", p = feature_path.display()),
    })?;
    let after_frontmatter = body_after_frontmatter(&body);
    let headings: HashSet<&str> = after_frontmatter
        .lines()
        .filter_map(|l| l.strip_prefix("## ").map(str::trim))
        .collect();
    for required in REQUIRED_H2_SECTIONS {
        if !headings.contains(*required) {
            return Ok(SpecCompleteness::MissingHeading(format!("## {required}")));
        }
    }
    Ok(SpecCompleteness::Complete)
}

fn body_after_frontmatter(body: &str) -> &str {
    let Some(rest) = body
        .strip_prefix("---\n")
        .or_else(|| body.strip_prefix("---\r\n"))
    else {
        return body;
    };
    match rest.find("\n---") {
        Some(end) => rest[end..].trim_start_matches("\n---").trim_start(),
        None => body,
    }
}

// ---------------------------------------------------------------------
// TC readiness — frontmatter runner check + body presence.
// ---------------------------------------------------------------------

/// Compose `tcs_linked` + `tcs_ready` for the feature. Iteration order
/// follows the feature's spec-declared `tests:` list so the planner
/// surfaces the first unready TC encountered — matches the table in
/// the FT-119 spec.
pub(super) fn tcs_linked_state(
    product_root: &Path,
    feature_id: &str,
) -> Result<TcsLinkedState, InspectError> {
    let feature_path = find_feature_file(product_root, feature_id).map_err(store)?;
    let body = fs::read_to_string(&feature_path).map_err(|e| InspectError::Store {
        detail: format!("read {p}: {e}", p = feature_path.display()),
    })?;
    let frontmatter = extract_frontmatter(&body).unwrap_or("");
    let tcs = extract_yaml_list(frontmatter, "tests");
    if tcs.is_empty() {
        return Ok(TcsLinkedState::NoneLinked);
    }
    for tc_id in &tcs {
        let Some(tc_path) = find_tc_file(product_root, tc_id) else {
            return Ok(TcsLinkedState::SomeUnready {
                problem_tc: tc_id.clone(),
                reason: "TC file missing".to_string(),
            });
        };
        let tc_body = fs::read_to_string(&tc_path).map_err(|e| InspectError::Store {
            detail: format!("read {p}: {e}", p = tc_path.display()),
        })?;
        let tc_fm = extract_frontmatter(&tc_body).unwrap_or("");
        if !has_non_empty_scalar(tc_fm, "runner") {
            return Ok(TcsLinkedState::SomeUnready {
                problem_tc: tc_id.clone(),
                reason: "runner missing".to_string(),
            });
        }
        if !has_non_empty_scalar(tc_fm, "runner-args") {
            return Ok(TcsLinkedState::SomeUnready {
                problem_tc: tc_id.clone(),
                reason: "runner-args missing".to_string(),
            });
        }
        if !has_non_empty_body(&tc_body) {
            return Ok(TcsLinkedState::SomeUnready {
                problem_tc: tc_id.clone(),
                reason: "body empty".to_string(),
            });
        }
    }
    Ok(TcsLinkedState::AllReady)
}

fn has_non_empty_scalar(frontmatter: &str, key: &str) -> bool {
    let prefix = format!("{key}:");
    for line in frontmatter.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return !rest.trim().trim_matches('"').is_empty();
        }
    }
    false
}

fn has_non_empty_body(body: &str) -> bool {
    let after = body_after_frontmatter(body);
    after.lines().any(|l| !l.trim().is_empty())
}

fn find_tc_file(product_root: &Path, tc_id: &str) -> Option<PathBuf> {
    let dir = product_root.join(".product").join("tests");
    let exact = dir.join(format!("{tc_id}.md"));
    if exact.is_file() {
        return Some(exact);
    }
    let prefix = format!("{tc_id}-");
    let entries = fs::read_dir(&dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(stem) = name.strip_suffix(".md") {
            if stem == tc_id || stem.starts_with(&prefix) {
                return Some(entry.path());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------
// Covering graph state — on-disk presence + pending_review session check.
// ---------------------------------------------------------------------

/// Resolve `(vgs_cover, vgs_accepted)` for a feature in the given env.
///
/// The split mirrors the FT-119 spec table:
///   * `Missing` — no live (non-superseded) on-disk VG covers the
///     feature AND no `pending_review` session is in flight for
///     `(feature, env)`. Triggers `DispatchVerifyGraphAuthor`.
///   * `PendingReview { graph_ids }` — at least one `pending_review`
///     session is in flight; the planner returns `Stuck` rather than
///     re-dispatching VGA (per FT-119 spec: "the DoR planner does not
///     auto-accept VGs out of pending_review").
///   * `AcceptedAll` — non-superseded on-disk VG(s) cover the feature
///     and no `pending_review` session is in flight.
///
/// We treat any pending session as gating: even if a covering VG
/// already exists on disk, surfacing the pending one in `Stuck`
/// surfaces the actionable item the operator needs to address.
pub(super) fn covering_graph_state(
    workdir: &Path,
    product_root: &Path,
    feature_id: &str,
    env_id: &str,
) -> Result<CoveringGraphState, InspectError> {
    let pending = pending_review_session_ids(workdir, feature_id, env_id)?;
    let covered = on_disk_covering_graph_present(workdir, product_root, feature_id)?;
    if !pending.is_empty() {
        return Ok(CoveringGraphState::PendingReview { graph_ids: pending });
    }
    if covered {
        Ok(CoveringGraphState::AcceptedAll)
    } else {
        Ok(CoveringGraphState::Missing)
    }
}

fn on_disk_covering_graph_present(
    workdir: &Path,
    product_root: &Path,
    feature_id: &str,
) -> Result<bool, InspectError> {
    use crate::core::ontology::verification_graph::io::from_turtle;
    use crate::core::store::{load_store_from_dump, orchestration_dump_path};
    use crate::core::verify::coverage::feature_resolver::{
        resolve_feature_tcs_short, tc_iri_for,
    };

    let tc_shorts =
        resolve_feature_tcs_short(product_root, feature_id).map_err(|e| InspectError::Store {
            detail: format!("resolve TCs for {feature_id}: {e}"),
        })?;
    if tc_shorts.is_empty() {
        return Ok(false);
    }
    let tc_iris: HashSet<String> = tc_shorts.iter().map(|s| tc_iri_for(s)).collect();

    let dump = orchestration_dump_path(workdir);
    let superseded = match load_store_from_dump(&dump) {
        Ok(store) => superseded_graph_shorts(&store),
        Err(_) => HashSet::new(),
    };

    let graph_dir = workdir.join(".dec").join("verify").join("graph");
    let Ok(read) = fs::read_dir(&graph_dir) else {
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
            .next_back()
            .unwrap_or_default()
            .to_string();
        if superseded.contains(&short) {
            continue;
        }
        let direct = tc_iris.contains(graph.verifies.0.as_str());
        let stepwise = graph.steps.iter().any(|step| {
            step.provides_evidence_for
                .iter()
                .any(|tc| tc_iris.contains(tc.as_str()))
        });
        if direct || stepwise {
            return Ok(true);
        }
    }
    Ok(false)
}

fn pending_review_session_ids(
    workdir: &Path,
    feature_id: &str,
    env_id: &str,
) -> Result<Vec<String>, InspectError> {
    use crate::core::store::{load_store_from_dump, orchestration_dump_path};

    let dump = orchestration_dump_path(workdir);
    let store = match load_store_from_dump(&dump) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };
    let feature_iri = format!("https://decision-cli.dev/ns/feature/{feature_id}");
    let bench_iri = format!("https://decision-cli.dev/ns/bench/{env_id}");
    let q = format!(
        r#"PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?session WHERE {{
  GRAPH ?g {{
    ?session a dec:Session ;
             dec:status "pending_review" ;
             dec:verifies <{feature_iri}> ;
             dec:bench <{bench_iri}> .
  }}
}}"#
    );
    let solutions = match store.query(&q) {
        Ok(QueryResults::Solutions(s)) => s,
        _ => return Ok(Vec::new()),
    };
    let mut out: Vec<String> = Vec::new();
    for sol in solutions.flatten() {
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("session") {
            out.push(short_session_id(n.as_str()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn short_session_id(session_iri: &str) -> String {
    let tail = session_iri
        .rsplit('/')
        .next()
        .unwrap_or(session_iri)
        .to_string();
    let head: String = tail.chars().take(8).collect();
    format!("pending-{head}")
}

fn superseded_graph_shorts(store: &oxigraph::store::Store) -> HashSet<String> {
    let q = r#"PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?graph WHERE { GRAPH ?g { ?graph dec:supersededBy ?_succ . } }"#;
    let mut out = HashSet::new();
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

// ---------------------------------------------------------------------
// Local frontmatter / file-discovery helpers (private to the module).
// ---------------------------------------------------------------------

fn store(e: impl std::fmt::Display) -> InspectError {
    InspectError::Store {
        detail: e.to_string(),
    }
}

fn find_feature_file(product_root: &Path, feature_id: &str) -> Result<PathBuf, String> {
    let dir = product_root.join(".product").join("features");
    let exact = dir.join(format!("{feature_id}.md"));
    if exact.is_file() {
        return Ok(exact);
    }
    let prefix = format!("{feature_id}-");
    let entries = fs::read_dir(&dir).map_err(|e| format!("read {d}: {e}", d = dir.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(stem) = name.strip_suffix(".md") {
            if stem == feature_id || stem.starts_with(&prefix) {
                return Ok(entry.path());
            }
        }
    }
    Err(format!("feature {feature_id} not found under {d}", d = dir.display()))
}

fn extract_frontmatter(body: &str) -> Option<&str> {
    let rest = body
        .strip_prefix("---\n")
        .or_else(|| body.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

fn extract_yaml_list(frontmatter: &str, key: &str) -> Vec<String> {
    let key_prefix = format!("{key}:");
    let mut lines = frontmatter.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        if !trimmed.starts_with(&key_prefix) {
            continue;
        }
        let rest = trimmed[key_prefix.len()..].trim();
        if rest.starts_with('[') && rest.ends_with(']') {
            return rest[1..rest.len() - 1]
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        let mut out = Vec::new();
        while let Some(next) = lines.peek() {
            let next_trim = next.trim_start();
            if let Some(item) = next_trim.strip_prefix("- ") {
                out.push(item.trim().trim_matches('"').to_string());
                lines.next();
            } else if next_trim.is_empty() {
                lines.next();
            } else {
                break;
            }
        }
        return out;
    }
    Vec::new()
}

fn which_on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// ---------------------------------------------------------------------
// Tests — frontmatter helpers + body parser. Live-store / live-product
// dimensions are exercised end-to-end via FT-119's existing TC runners
// once they wire production through, which is the next iteration.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_frontmatter_handles_block_yaml() {
        let body = "---\nstatus: complete\ntests:\n- TC-001\n- TC-002\n---\n\n# body\n";
        let fm = extract_frontmatter(body).unwrap();
        assert!(fm.contains("status: complete"));
        assert_eq!(extract_yaml_list(fm, "tests"), vec!["TC-001", "TC-002"]);
    }

    #[test]
    fn extract_yaml_list_handles_inline_form() {
        let fm = "tests: [TC-001, \"TC-002\"]";
        assert_eq!(extract_yaml_list(fm, "tests"), vec!["TC-001", "TC-002"]);
    }

    #[test]
    fn extract_yaml_list_returns_empty_on_missing_key() {
        let fm = "status: planned";
        assert!(extract_yaml_list(fm, "tests").is_empty());
    }

    #[test]
    fn body_after_frontmatter_strips_frontmatter_block() {
        let body = "---\nid: FT-001\n---\n\n## Description\nfoo\n";
        let after = body_after_frontmatter(body);
        assert!(after.starts_with("## Description"));
    }

    #[test]
    fn body_after_frontmatter_passes_through_when_absent() {
        let body = "## Description\nfoo\n";
        assert_eq!(body_after_frontmatter(body), body);
    }

    #[test]
    fn has_non_empty_scalar_distinguishes_blank_from_set() {
        let fm = "runner: cargo-test\nrunner-args: \nstatus: planned";
        assert!(has_non_empty_scalar(fm, "runner"));
        assert!(!has_non_empty_scalar(fm, "runner-args"));
        assert!(has_non_empty_scalar(fm, "status"));
    }

    #[test]
    fn has_non_empty_body_detects_body_content() {
        let with_body = "---\nid: X\n---\n## Claim\nbody text\n";
        let only_fm = "---\nid: X\n---\n";
        assert!(has_non_empty_body(with_body));
        assert!(!has_non_empty_body(only_fm));
    }

    #[test]
    fn variant_key_orders_adr_gaps_before_domain_gaps() {
        let mut gaps = vec![
            PreflightGap::UncoveredDomain("storage".to_string()),
            PreflightGap::UnacknowledgedAdr("ADR-070".to_string()),
            PreflightGap::UnacknowledgedAdr("ADR-013".to_string()),
            PreflightGap::UncoveredDomain("api".to_string()),
        ];
        sort_gaps(&mut gaps);
        assert_eq!(
            gaps,
            vec![
                PreflightGap::UnacknowledgedAdr("ADR-013".to_string()),
                PreflightGap::UnacknowledgedAdr("ADR-070".to_string()),
                PreflightGap::UncoveredDomain("api".to_string()),
                PreflightGap::UncoveredDomain("storage".to_string()),
            ]
        );
    }

    #[test]
    fn short_session_id_truncates_uuid_tail() {
        let iri = "urn:dec:session/verify-graph-author-dispatch/FT-119/BNCH-002/abcdef0123-4567-89ab-cdef-0123456789ab";
        let short = short_session_id(iri);
        assert_eq!(short, "pending-abcdef01");
    }
}
