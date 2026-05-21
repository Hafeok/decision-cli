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

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_env::from_turtle as env_from_turtle;
use crate::core::verify::coverage::feature_resolver::{resolve_feature_tcs_short, tc_iri_for};
use crate::core::verify::matcher::MatchReport;
use crate::core::vocab::{
    IRI_DEC_ENV_PREFIX, IRI_DEC_VERIFY_GRAPH_PREFIX, STEP_KIND_CAPTURE, STEP_KIND_FILE_ASSERTION,
    STEP_KIND_HTTP_REQUEST, STEP_KIND_SHELL_COMMAND, STEP_KIND_SPARQL_ASSERTION,
    STEP_KIND_WAIT_FOR,
};

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
}

/// Target environment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvRecord {
    /// Short env id (e.g. `ENV-1` or `ENV-001-ephemeral-cli`).
    pub id: String,
    /// Env kind (e.g. `ephemeral-tempdir`).
    pub env_type: String,
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

/// Assemble the bundle for `(feature, env, match_report)`.
///
/// `product_root` is where `.product/features/` lives (already resolved
/// by the caller); `workdir` is where `.dec/verify/env/` lives.
pub fn assemble_bundle(
    workdir: &Path,
    product_root: &Path,
    feature_id: &str,
    env_short: &str,
    match_report: &MatchReport,
) -> Result<VerifyGraphAuthorInputJson, HandlerError> {
    let feature_spec = load_feature_body(product_root, feature_id)?;
    let tc_shorts = resolve_feature_tcs_short(product_root, feature_id).map_err(|e| {
        HandlerError::Internal {
            detail: format!("bundle: resolving TC list: {e}"),
        }
    })?;
    let relevant_tcs = tc_shorts
        .iter()
        .map(|t| TcRecord {
            id: t.clone(),
            title: String::new(),
            body: String::new(),
        })
        .collect();
    let target_environment = load_env_record(workdir, env_short)?;
    let candidate_graphs = candidate_records_from_report(match_report);
    let step_vocabulary = default_step_vocabulary();
    let mut bundle = VerifyGraphAuthorInputJson {
        feature_id: feature_id.to_string(),
        feature_spec,
        relevant_tcs,
        target_environment,
        candidate_graphs,
        step_vocabulary,
        bundle_hash: String::new(),
    };
    bundle.bundle_hash = compute_bundle_hash(&bundle);
    Ok(bundle)
}

fn load_feature_body(product_root: &Path, feature_id: &str) -> Result<String, HandlerError> {
    let features_dir = product_root.join(".product").join("features");
    if !features_dir.exists() {
        return Err(HandlerError::ArtifactNotFound {
            kind: "Feature".to_string(),
            id: feature_id.to_string(),
        });
    }
    let exact = features_dir.join(format!("{feature_id}.md"));
    if exact.is_file() {
        return fs::read_to_string(&exact).map_err(|e| HandlerError::Internal {
            detail: format!("bundle: reading {p}: {e}", p = exact.display()),
        });
    }
    let prefix = format!("{feature_id}-");
    let entries = fs::read_dir(&features_dir).map_err(|e| HandlerError::Internal {
        detail: format!(
            "bundle: reading {d}: {e}",
            d = features_dir.display()
        ),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| HandlerError::Internal {
            detail: format!(
                "bundle: walking {d}: {e}",
                d = features_dir.display()
            ),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        if stem == feature_id || stem.starts_with(&prefix) {
            return fs::read_to_string(entry.path()).map_err(|e| HandlerError::Internal {
                detail: format!("bundle: reading feature file: {e}"),
            });
        }
    }
    Err(HandlerError::ArtifactNotFound {
        kind: "Feature".to_string(),
        id: feature_id.to_string(),
    })
}

fn load_env_record(workdir: &Path, env_short: &str) -> Result<EnvRecord, HandlerError> {
    let env_dir = workdir.join(".dec").join("verify").join("env");
    let env_path = env_dir.join(format!("{env_short}.ttl"));
    if !env_path.is_file() {
        return Err(HandlerError::ArtifactNotFound {
            kind: "VerificationEnvironment".to_string(),
            id: env_short.to_string(),
        });
    }
    let env = env_from_turtle(&env_path).map_err(|e| HandlerError::Internal {
        detail: format!("bundle: parsing env {p}: {e}", p = env_path.display()),
    })?;
    Ok(EnvRecord {
        id: env_short.to_string(),
        env_type: env.env_type.clone(),
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
            let short_id = g
                .id
                .strip_prefix(IRI_DEC_VERIFY_GRAPH_PREFIX)
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

/// The six FT-036 seed step kinds plus their `required_ops` (per
/// ADR-028 §Typed step vocabulary). Mirrors `core::verify::safety`'s
/// op-derivation table.
fn default_step_vocabulary() -> Vec<StepKindRecord> {
    vec![
        StepKindRecord {
            kind: STEP_KIND_SHELL_COMMAND.to_string(),
            required_ops: vec!["shell".to_string(), "filesystem".to_string()],
            fields_schema: serde_json::json!({}),
            description: "Run a shell command; assert exit code.".to_string(),
        },
        StepKindRecord {
            kind: STEP_KIND_SPARQL_ASSERTION.to_string(),
            required_ops: vec!["sparql-local".to_string()],
            fields_schema: serde_json::json!({}),
            description: "Run a SPARQL query; assert row count.".to_string(),
        },
        StepKindRecord {
            kind: STEP_KIND_FILE_ASSERTION.to_string(),
            required_ops: vec!["filesystem".to_string()],
            fields_schema: serde_json::json!({}),
            description: "Assert file existence or content.".to_string(),
        },
        StepKindRecord {
            kind: STEP_KIND_HTTP_REQUEST.to_string(),
            required_ops: vec!["http".to_string()],
            fields_schema: serde_json::json!({}),
            description: "Make an HTTP call; assert status.".to_string(),
        },
        StepKindRecord {
            kind: STEP_KIND_WAIT_FOR.to_string(),
            required_ops: Vec::new(),
            fields_schema: serde_json::json!({}),
            description: "Poll a sub-condition with timeout.".to_string(),
        },
        StepKindRecord {
            kind: STEP_KIND_CAPTURE.to_string(),
            required_ops: Vec::new(),
            fields_schema: serde_json::json!({}),
            description: "Bind a prior step's stdout/result to a name.".to_string(),
        },
    ]
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
    iri.strip_prefix(IRI_DEC_ENV_PREFIX)
        .unwrap_or(iri)
        .to_string()
}
