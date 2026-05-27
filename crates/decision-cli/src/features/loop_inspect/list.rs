//! `dec loop list` — overview across all features.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::render::OutputFormat;
use crate::core::feedback::read::list_by_class;
use crate::core::handler::Error as HandlerError;
use crate::core::store::{load_store_from_dump, orchestration_dump_path};

/// Which feedback states the list should include.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateFilter {
    /// Only features with at least one open (`produced | routed | received`) defect.
    Open,
    /// Only features whose feedback is entirely closed (`addressed | closed`).
    Closed,
    /// All features that appear in any feedback's source-artifact chain.
    All,
}

impl Default for StateFilter {
    fn default() -> Self {
        Self::Open
    }
}

impl StateFilter {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

/// Wire request.
#[derive(Debug, Clone)]
pub struct LoopListRequest {
    pub workdir: PathBuf,
    pub product_root: Option<PathBuf>,
    pub state: StateFilter,
    pub format: OutputFormat,
}

/// One row of the rollup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopFeatureRow {
    pub feature_id: String,
    pub open_count: usize,
    pub closed_count: usize,
    pub last_emitted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopListResponse {
    pub rows: Vec<LoopFeatureRow>,
    /// Count of defect-feedback artifacts whose `sourceArtifact` could
    /// not be mapped to a feature (e.g. a catalog gap or unscoped TC).
    pub unscoped_count: usize,
}

/// Single handler.
pub fn run(req: &LoopListRequest) -> Result<LoopListResponse, HandlerError> {
    let product_root = req
        .product_root
        .clone()
        .unwrap_or_else(|| req.workdir.clone());

    let dump = orchestration_dump_path(&req.workdir);
    let store = load_store_from_dump(&dump).map_err(|e| HandlerError::Internal {
        detail: format!(
            "loop list: opening orchestration store at {p}: {e}",
            p = dump.display()
        ),
    })?;
    let defects = list_by_class(&store, "defect").map_err(|e| HandlerError::Internal {
        detail: format!("loop list: reading defect feedback: {e}"),
    })?;

    // Build a TC-short → owning-feature lookup by walking
    // `.product/tests/TC-NNN-*.md` once. The product graph would be the
    // canonical source, but the markdown frontmatter is on-disk
    // authoritative per the project's product-cli convention.
    let tc_to_feature = build_tc_to_feature_map(&product_root);

    #[derive(Default)]
    struct Tally {
        open: usize,
        closed: usize,
        last_emitted: Option<String>,
    }
    let mut by_feature: HashMap<String, Tally> = HashMap::new();
    let mut unscoped: usize = 0;

    for fb in defects {
        let Some(source) = fb.source_artifact.as_ref() else {
            unscoped += 1;
            continue;
        };
        let tc_iri = source.as_str();
        let tc_short = tc_iri
            .strip_prefix("https://decision-cli.dev/ns/tc/")
            .unwrap_or(tc_iri);
        let feature_id = match tc_to_feature.get(tc_short) {
            Some(f) => f.clone(),
            None => {
                unscoped += 1;
                continue;
            }
        };
        let entry = by_feature.entry(feature_id).or_default();
        if is_open(&fb.lifecycle_state) {
            entry.open += 1;
        } else {
            entry.closed += 1;
        }
        let stamp = fb.routed_at.clone();
        if let Some(s) = stamp {
            if entry.last_emitted.as_ref().is_none_or(|cur| *cur < s) {
                entry.last_emitted = Some(s);
            }
        }
    }

    let mut rows: Vec<LoopFeatureRow> = by_feature
        .into_iter()
        .filter_map(|(feature_id, tally)| {
            let include = match req.state {
                StateFilter::All => true,
                StateFilter::Open => tally.open > 0,
                StateFilter::Closed => tally.open == 0 && tally.closed > 0,
            };
            if !include {
                return None;
            }
            Some(LoopFeatureRow {
                feature_id,
                open_count: tally.open,
                closed_count: tally.closed,
                last_emitted_at: tally.last_emitted,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.open_count
            .cmp(&a.open_count)
            .then(b.last_emitted_at.clone().unwrap_or_default()
                .cmp(&a.last_emitted_at.clone().unwrap_or_default()))
            .then(a.feature_id.cmp(&b.feature_id))
    });

    Ok(LoopListResponse {
        rows,
        unscoped_count: unscoped,
    })
}

fn is_open(state: &str) -> bool {
    matches!(state, "produced" | "routed" | "received")
}

/// Build a `TC-NNN → FT-NNN` map by walking `.product/tests/` and
/// reading each TC's `validates.features` frontmatter.
fn build_tc_to_feature_map(product_root: &std::path::Path) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let tests_dir = product_root.join(".product").join("tests");
    let Ok(read) = std::fs::read_dir(&tests_dir) else {
        return out;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        // Files are named TC-NNN-<slug>.md; extract the TC short id.
        let tc_short = name
            .split('-')
            .take(2)
            .collect::<Vec<_>>()
            .join("-");
        if !tc_short.starts_with("TC-") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(feature_id) = first_validated_feature(&body) {
            out.insert(tc_short, feature_id);
        }
    }
    out
}

fn first_validated_feature(body: &str) -> Option<String> {
    let mut in_fm = false;
    let mut in_features = false;
    for line in body.lines() {
        let trimmed = line.trim_end();
        if trimmed == "---" {
            if in_fm {
                break;
            }
            in_fm = true;
            continue;
        }
        if !in_fm {
            continue;
        }
        if trimmed.starts_with("validates:") {
            in_features = false;
            continue;
        }
        if trimmed.trim_start().starts_with("features:") {
            in_features = true;
            continue;
        }
        if in_features {
            let stripped = trimmed.trim_start();
            if let Some(rest) = stripped.strip_prefix("- ") {
                let ft = rest.trim().trim_matches(|c| c == '"' || c == '\'');
                if ft.starts_with("FT-") {
                    return Some(ft.to_string());
                }
                return None;
            } else if !stripped.is_empty() && !stripped.starts_with(' ') {
                // exited the features sub-list without a hit
                in_features = false;
            }
        }
    }
    None
}
