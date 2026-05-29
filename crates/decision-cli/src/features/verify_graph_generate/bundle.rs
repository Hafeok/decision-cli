//! Bundle assembly for the verify-graph-author worker (FT-049 / ADR-030).
//!
//! Builds the `VerifyGraphAuthorInput` envelope from:
//!
//! - the feature id + feature_spec body (read from `.product/features/`),
//! - the TC ids the feature lists (resolved via the coverage primitive),
//! - the target env record (read from `.dec/verify/env/`),
//! - the candidate graphs from the matcher's `MatchReport.graphs`,
//! - the step vocabulary (six seed kinds + their `required_ops`),
//! - a deterministic `bundle_hash` over the canonical serialisation.
//!
//! The shape mirrors the Python worker's `VerifyGraphAuthorInput`
//! pydantic model so the JSON round-trip is byte-equal.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::dispatch::capability_resolver::ResolvedCapability;
use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_bench::{
    from_turtle as bench_from_turtle, VerificationBench,
};
use crate::core::store::{load_store_from_dump, orchestration_dump_path};
use crate::core::verify::coverage::feature_resolver::{resolve_feature_tcs_short, tc_iri_for};
use crate::core::verify::matcher::MatchReport;
use crate::core::vocab::{IRI_DEC_BENCH_PREFIX, IRI_DEC_VERIFY_GRAPH_PREFIX};

use super::enrichment::{
    assemble_enrichment, read_concrete_capabilities_from_turtle, EnrichmentFields,
    DEFAULT_DEC_VERSION,
};
use super::step_vocabulary::default_step_vocabulary;

/// Default `max_tokens` requested from the worker when the resolved
/// capability has no explicit ceiling. Matches the Python pydantic
/// shape's default (FT-064 `VerifyGraphAuthorInput.max_tokens = 4096`).
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Bundle envelope sent to the worker on stdin (FT-049 §Behaviour).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyGraphAuthorInputJson {
    /// Short feature id (e.g. `FT-049`).
    pub feature_id: String,
    /// Raw feature_spec markdown body.
    pub feature_spec: String,
    /// Test criteria the proposal must cover.
    pub relevant_tcs: Vec<TcRecord>,
    /// The single target environment.
    pub target_environment: EnvRecord,
    /// Existing graphs touching any of the feature's TCs in this env.
    pub candidate_graphs: Vec<ExistingGraphRecord>,
    /// Step kinds the worker is allowed to propose.
    pub step_vocabulary: Vec<StepKindRecord>,
    /// SHA-256 hex of the canonical bundle (echoed in the proposal).
    pub bundle_hash: String,
    /// FT-068 / ADR-033 — endpoint discriminator resolved by the
    /// dispatcher (`"anthropic"` | `"scaleway"`). Mirrors the field on
    /// the Python pydantic model.
    pub endpoint: String,
    /// FT-068 — exact provider model identifier from the resolved
    /// capability (e.g. `qwen3-coder-30b-a3b-instruct`). The worker
    /// passes this straight to the SDK.
    pub model_id: String,
    /// FT-068 — capability-resolved parameters forwarded to the router
    /// untouched. Empty object today; FT-063's `reasoning_effort` and
    /// future per-capability knobs flow through here.
    pub parameters: serde_json::Value,
    /// FT-068 — maximum output tokens. Sourced from the resolved
    /// capability's `max_output`, capped at the Python pydantic
    /// schema's bounds (256..=64_000).
    pub max_tokens: u32,
    /// ADR-066 / FT-102 — the five bundle-completeness fields the
    /// verify-graph-author worker reads as the ground truth for its
    /// proposal. Populated by SPARQL queries against the catalog.
    /// `Default::default()` for legacy / pre-FT-102 paths.
    #[serde(default)]
    pub enrichment: EnrichmentFields,
    /// FT-107 — runtime evidence the existing covering graph is broken.
    /// Empty for the green path (matcher short-circuit); non-empty when
    /// the orchestrator routes a re-authoring dispatch.
    #[serde(default)]
    pub defect_feedback: Vec<super::defect_feedback::DefectFeedbackRecord>,
}

/// One test-criterion record in the bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcRecord {
    /// Short TC id (e.g. `TC-013`).
    pub id: String,
    /// TC title; empty string when not resolvable.
    #[serde(default)]
    pub title: String,
    /// Markdown body; empty when not resolvable.
    #[serde(default)]
    pub body: String,
    /// Runner name from TC frontmatter (e.g. `bash`, `cargo-test`,
    /// `pytest`). Empty when the TC declares none — in that case
    /// the verifier has to compose its own command from the body.
    #[serde(default)]
    pub runner: String,
    /// Runner arguments from TC frontmatter (e.g.
    /// `tests/scripts/tc-007-unauthorized-goal.sh`). Empty when no
    /// runner is declared.
    #[serde(default)]
    pub runner_args: String,
    /// Runner timeout (e.g. `30`, `30s`). Empty when no timeout is
    /// declared. The verifier doesn't translate this directly into
    /// a step field — it's informational.
    #[serde(default)]
    pub runner_timeout: String,
}

/// Target environment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvRecord {
    /// Short env id (e.g. `ENV-1` or `BNCH-001-ephemeral-cli`).
    pub id: String,
    /// Env kind (e.g. `ephemeral-tempdir`).
    pub bench_type: String,
    /// Safety classification.
    #[serde(default)]
    pub safety_class: String,
    /// Allowed ops the env permits.
    #[serde(default)]
    pub allowed_ops: Vec<String>,
    /// Optional endpoint URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// A candidate existing graph the worker may consider for `Match`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingGraphRecord {
    /// Short graph id (e.g. `VG-007`).
    pub id: String,
    /// What this graph verifies (feature or TC).
    #[serde(default)]
    pub verifies: String,
    /// TC ids this graph covers in the target env.
    #[serde(default)]
    pub covers: Vec<String>,
    /// Condensed step summaries (kind + provides_evidence_for).
    #[serde(default)]
    pub step_summaries: Vec<StepSummaryRecord>,
}

/// Condensed view of one step in a candidate graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSummaryRecord {
    /// Step kind tag.
    pub step_type: String,
    /// Short description (worker can ignore).
    #[serde(default)]
    pub summary: String,
    /// TC ids this step provides evidence for.
    #[serde(default)]
    pub provides_evidence_for: Vec<String>,
}

/// One step kind in the worker's vocabulary (with required ops).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepKindRecord {
    /// Kind name (one of the six seed kinds).
    pub kind: String,
    /// Required ops at execution time.
    #[serde(default)]
    pub required_ops: Vec<String>,
    /// Optional fields-schema. Empty object when not declared.
    #[serde(default)]
    pub fields_schema: serde_json::Value,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
}

/// Assemble the bundle for `(feature, env, match_report, capability)`.
///
/// `product_root` is where `.product/features/` lives (already resolved
/// by the caller); `workdir` is where `.dec/verify/env/` lives.
/// `capability` is the FT-068 resolved capability the dispatcher pinned
/// for the verify-graph-author role — `endpoint`, `model_id`, and
/// `max_tokens` propagate verbatim into the bundle.
pub fn assemble_bundle(
    workdir: &Path,
    product_root: &Path,
    feature_id: &str,
    env_short: &str,
    match_report: &MatchReport,
    capability: &ResolvedCapability,
) -> Result<VerifyGraphAuthorInputJson, HandlerError> {
    let feature_spec = load_feature_body(product_root, feature_id)?;
    let tc_shorts = resolve_feature_tcs_short(product_root, feature_id).map_err(|e| {
        HandlerError::Internal {
            detail: format!("bundle: resolving TC list: {e}"),
        }
    })?;
    let relevant_tcs = tc_shorts
        .iter()
        .map(|t| load_tc_record(product_root, t))
        .collect();
    let target_environment = load_env_record(workdir, env_short)?;
    let env_struct = load_env_struct(workdir, env_short)?;
    let candidate_graphs = candidate_records_from_report(match_report);
    let step_vocabulary = default_step_vocabulary();
    let max_tokens = clamp_max_tokens(capability.max_output);
    let enrichment = assemble_enrichment_for(workdir, env_struct.as_ref(), env_short)?;
    let defect_feedback =
        super::defect_feedback::load_for(workdir, feature_id, env_short);
    let mut bundle = VerifyGraphAuthorInputJson {
        feature_id: feature_id.to_string(),
        feature_spec,
        relevant_tcs,
        target_environment,
        candidate_graphs,
        step_vocabulary,
        bundle_hash: String::new(),
        endpoint: capability.endpoint.as_str().to_string(),
        model_id: capability.model_identifier.clone(),
        parameters: serde_json::json!({}),
        max_tokens,
        enrichment,
        defect_feedback,
    };
    bundle.bundle_hash = compute_bundle_hash(&bundle);
    Ok(bundle)
}

/// Helper that builds the env struct for the enrichment query path.
/// Mirrors [`load_env_record`] but returns the full
/// `VerificationBench` rather than the wire-shape record.
fn load_env_struct(
    workdir: &Path,
    env_short: &str,
) -> Result<Option<VerificationBench>, HandlerError> {
    let env_path = env_path_for(workdir, env_short);
    if !env_path.is_file() {
        return Ok(None);
    }
    let env = bench_from_turtle(&env_path).map_err(|e| HandlerError::Internal {
        detail: format!("bundle: parsing env {p}: {e}", p = env_path.display()),
    })?;
    Ok(Some(env))
}

fn env_path_for(workdir: &Path, env_short: &str) -> std::path::PathBuf {
    // FT-112 renamed env/ → bench/; this loader was missed (see also
    // load_env_record at the bottom of this file).
    workdir
        .join(".dec")
        .join("verify")
        .join("bench")
        .join(format!("{env_short}.ttl"))
}

/// Run the catalog SPARQL queries that populate the five ADR-066 fields.
/// The orchestration store is loaded from `<workdir>/.dec/store/`.
pub fn assemble_enrichment_for(
    workdir: &Path,
    env: Option<&VerificationBench>,
    env_short: &str,
) -> Result<EnrichmentFields, HandlerError> {
    let dump_path = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("enrichment: opening orchestration store: {e}"),
    })?;
    let env_path = env_path_for(workdir, env_short);
    let concrete = read_concrete_capabilities_from_turtle(&env_path)?;
    let dec_version = resolved_dec_version();
    assemble_enrichment(&store, env, concrete.as_ref(), &dec_version)
}

fn resolved_dec_version() -> String {
    std::env::var("DEC_VERIFY_BUNDLE_DEC_VERSION")
        .unwrap_or_else(|_| DEFAULT_DEC_VERSION.to_string())
}

/// Clamp the resolved capability's `max_output` into the Python worker's
/// pydantic-enforced `[256, 64_000]` window. Falls back to
/// [`DEFAULT_MAX_TOKENS`] when the capability declares `0` (e.g.
/// embedding capabilities that should never be used here, but defensive).
fn clamp_max_tokens(max_output: u32) -> u32 {
    if max_output == 0 {
        return DEFAULT_MAX_TOKENS;
    }
    max_output.clamp(256, 64_000)
}

fn load_feature_body(product_root: &Path, feature_id: &str) -> Result<String, HandlerError> {
    let features_dir = product_root.join(".product").join("features");
    if !features_dir.exists() {
        return Err(feature_not_found(feature_id));
    }
    let exact = features_dir.join(format!("{feature_id}.md"));
    if exact.is_file() {
        return read_feature_file(&exact);
    }
    match find_prefixed_feature_path(&features_dir, feature_id)? {
        Some(path) => read_feature_file(&path),
        None => Err(feature_not_found(feature_id)),
    }
}

fn feature_not_found(feature_id: &str) -> HandlerError {
    HandlerError::ArtifactNotFound {
        kind: "Feature".to_string(),
        id: feature_id.to_string(),
    }
}

fn read_feature_file(path: &Path) -> Result<String, HandlerError> {
    fs::read_to_string(path).map_err(|e| HandlerError::Internal {
        detail: format!("bundle: reading {p}: {e}", p = path.display()),
    })
}

/// FT-107.F — surface each TC's markdown body to the worker. Without
/// this the worker only sees the TC short id, leaving it to guess the
/// claim the graph must produce evidence for. The body includes the
/// frontmatter (title, validates) and the Description / Scenarios /
/// Boundary sections the spec author wrote.
///
/// Best-effort: if the TC file is missing the record returns id-only
/// with empty title/body so the bundle still assembles. Lookup mirrors
/// `load_feature_body`: exact match first, then prefix scan.
fn load_tc_record(product_root: &Path, tc_id: &str) -> TcRecord {
    let empty = |id: &str| TcRecord {
        id: id.to_string(),
        title: String::new(),
        body: String::new(),
        runner: String::new(),
        runner_args: String::new(),
        runner_timeout: String::new(),
    };
    let tests_dir = product_root.join(".product").join("tests");
    if !tests_dir.is_dir() {
        return empty(tc_id);
    }
    let path = tests_dir.join(format!("{tc_id}.md"));
    let path = if path.is_file() {
        Some(path)
    } else {
        find_prefixed_tc_path(&tests_dir, tc_id)
    };
    let Some(path) = path else {
        return empty(tc_id);
    };
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return empty(tc_id),
    };
    let frontmatter = extract_tc_frontmatter(&raw);
    TcRecord {
        id: tc_id.to_string(),
        title: frontmatter.title,
        body: raw,
        runner: frontmatter.runner,
        runner_args: frontmatter.runner_args,
        runner_timeout: frontmatter.runner_timeout,
    }
}

/// Parsed TC frontmatter — only the fields the verifier needs.
#[derive(Default)]
struct TcFrontmatter {
    title: String,
    runner: String,
    runner_args: String,
    runner_timeout: String,
}

fn extract_tc_frontmatter(body: &str) -> TcFrontmatter {
    let mut in_frontmatter = false;
    let mut out = TcFrontmatter::default();
    for line in body.lines() {
        if line.trim() == "---" {
            if in_frontmatter {
                return out;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        let trimmed_value = |rest: &str| -> String {
            rest.trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string()
        };
        if let Some(rest) = line.strip_prefix("title:") {
            out.title = trimmed_value(rest);
        } else if let Some(rest) = line.strip_prefix("runner:") {
            out.runner = trimmed_value(rest);
        } else if let Some(rest) = line.strip_prefix("runner-args:") {
            out.runner_args = trimmed_value(rest);
        } else if let Some(rest) = line.strip_prefix("runner-timeout:") {
            out.runner_timeout = trimmed_value(rest);
        }
    }
    out
}

fn find_prefixed_tc_path(tests_dir: &Path, tc_id: &str) -> Option<std::path::PathBuf> {
    let prefix = format!("{tc_id}-");
    let entries = fs::read_dir(tests_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with(&prefix) && name.ends_with(".md") {
            return Some(entry.path());
        }
    }
    None
}


fn find_prefixed_feature_path(
    features_dir: &Path,
    feature_id: &str,
) -> Result<Option<std::path::PathBuf>, HandlerError> {
    let prefix = format!("{feature_id}-");
    let entries = fs::read_dir(features_dir).map_err(|e| HandlerError::Internal {
        detail: format!("bundle: reading {d}: {e}", d = features_dir.display()),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| HandlerError::Internal {
            detail: format!("bundle: walking {d}: {e}", d = features_dir.display()),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        if stem == feature_id || stem.starts_with(&prefix) {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

fn load_env_record(workdir: &Path, env_short: &str) -> Result<EnvRecord, HandlerError> {
    // FT-112 renamed `.dec/verify/env/` to `.dec/verify/bench/`. This
    // loader was missed in the original migration; FT-117 surfaced the
    // gap when drives kept failing at iteration 0 with "not found".
    let env_dir = workdir.join(".dec").join("verify").join("bench");
    let env_path = env_dir.join(format!("{env_short}.ttl"));
    if !env_path.is_file() {
        return Err(HandlerError::ArtifactNotFound {
            kind: "VerificationBench".to_string(),
            id: env_short.to_string(),
        });
    }
    let env = bench_from_turtle(&env_path).map_err(|e| HandlerError::Internal {
        detail: format!("bundle: parsing env {p}: {e}", p = env_path.display()),
    })?;
    Ok(EnvRecord {
        id: env_short.to_string(),
        bench_type: env.bench_type.clone(),
        safety_class: env.safety_class.as_str().to_string(),
        allowed_ops: env.allowed_ops.clone(),
        endpoint: env.endpoint.clone(),
    })
}

fn candidate_records_from_report(report: &MatchReport) -> Vec<ExistingGraphRecord> {
    report
        .graphs
        .iter()
        .map(|g| {
            let short_id =
                g.id.strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
                    .unwrap_or(g.id.as_str())
                    .to_string();
            let covers_short = g
                .covers
                .iter()
                .map(|t| {
                    t.strip_prefix("https://decision-cli.dev/ns/tc/")
                        .unwrap_or(t.as_str())
                        .to_string()
                })
                .collect();
            ExistingGraphRecord {
                id: short_id,
                verifies: g.verifies.0.as_str().to_string(),
                covers: covers_short,
                step_summaries: Vec::new(),
            }
        })
        .collect()
}

/// FT-110 worker-quality follow-up: public re-export of the bundle-hash
/// computation so the validator-retry path can recompute after mutating
/// `enrichment.bundle_metadata.warnings`. Mirrors the private signature.
pub fn compute_bundle_hash_pub(bundle: &VerifyGraphAuthorInputJson) -> String {
    compute_bundle_hash(bundle)
}

fn compute_bundle_hash(bundle: &VerifyGraphAuthorInputJson) -> String {
    let mut to_hash = bundle.clone();
    to_hash.bundle_hash = String::new();
    let serialised = serde_json::to_string(&to_hash).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(serialised.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Convert a short TC id list into IRI form (helper for the persistence
/// path).
#[must_use]
pub fn tc_short_to_iri(short: &str) -> String {
    if short.starts_with("https://") {
        short.to_string()
    } else {
        tc_iri_for(short)
    }
}

/// Strip the `dec:` env-IRI prefix, returning the short id when the
/// input is an IRI, or the input verbatim otherwise.
#[must_use]
pub fn env_iri_to_short(iri: &str) -> String {
    iri.strip_prefix(IRI_DEC_BENCH_PREFIX)
        .unwrap_or(iri)
        .to_string()
}
