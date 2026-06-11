//! ADR-066 bundle enrichment — the five fields the verify-graph-author worker reads.
//!
//! Per ADR-066 §Decision the bundle assembler reads these from catalog
//! artifacts via SPARQL queries (FT-101's `dec:CapabilityReference`,
//! `dec:OntologyDescription`, `dec:ExemplarGraph`) plus the target env's
//! optional `dec:concreteCapabilities` block. The chokepoint validator
//! ([`super::validator`]) treats every fact in the bundle as the
//! ground truth the worker may reference.
//!
//! The fields are populated by [`assemble_enrichment`], called from the
//! main `assemble_bundle` path in [`super::bundle`]. When the catalog
//! is missing mandatory artifacts (`cli_surface`, `ontology_vocabulary`)
//! the assembler returns `Error::CatalogIncomplete` before the worker is
//! dispatched (TC-171).

use std::fs;
use std::path::Path;

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{GraphName, NamedNode, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::handler::Error as HandlerError;
use crate::core::ontology::verification_bench::VerificationBench;
use crate::core::vocab::IRI_DEC_GRAPH_CATALOG;

/// Default `dec --version` the bundle assembler matches against when
/// filtering `dec:CapabilityReference` artifacts. Lives here as a
/// constant so tests can pin the value deterministically without
/// shelling out for `dec --version`.
pub const DEFAULT_DEC_VERSION: &str = "0.3.0";

/// The five ADR-066 fields plus the metadata block. Serialised verbatim
/// into the worker's input envelope alongside the FT-048 fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnrichmentFields {
    /// CLI surface the worker may invoke from `shell-command` steps.
    pub cli_surface: CliSurface,
    /// Ontology vocabulary the worker may reference in `sparql-assertion`.
    pub ontology_vocabulary: OntologyVocabulary,
    /// Store query surface — how to address the orchestration store
    /// from inside a step.
    pub store_query_surface: StoreQuerySurface,
    /// Env capabilities — binaries, writable paths, hosts, env vars.
    pub env_capabilities: EnvCapabilities,
    /// Curated exemplar graphs for the env's safety class.
    pub exemplar_graphs: Vec<ExemplarRecord>,
    /// Replay-determinism metadata (catalog hashes + warnings).
    pub bundle_metadata: BundleMetadata,
}

/// CLI surface — every `dec:CapabilityReference` that is non-superseded
/// and matches the running `dec` binary's version.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliSurface {
    /// Structured per-command records.
    pub commands: Vec<CliCommand>,
    /// Short `dec <subcommand>` strings (e.g. `dec verify graph new`).
    /// Mirror of `commands[*].command` but flattened for cheap lookup.
    pub dec_subcommands: Vec<String>,
    /// The dec version this surface was resolved against (e.g. `0.3.0`).
    pub capability_version: String,
    /// Valid value-stream template names for `dec init --template
    /// <name>`. Derived from the bundled stream assets at build time
    /// — the verifier MUST pick from this list when authoring an
    /// init step. Hallucinated template names (e.g.
    /// `decision-cli-development`) blow up step 0 with exit 1.
    #[serde(default)]
    pub init_templates: Vec<String>,
}

/// One structured command from a `CapabilityReference`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliCommand {
    /// Canonical command string (e.g. `dec verify graph new`).
    pub command: String,
    /// Capability version (e.g. `0.3.0`).
    pub capability_version: String,
    /// Source CR id (e.g. `CR-001`).
    pub source_cr: String,
}

/// Ontology vocabulary surface — namespace + classes + canonical
/// predicates the worker may reference.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OntologyVocabulary {
    /// Canonical dec namespace (e.g. `https://decision-cli.dev/ns#`).
    pub namespace: String,
    /// Prefix alias (usually `dec`).
    pub prefix: String,
    /// All namespaces the worker may reference (the dec namespace plus
    /// the W3C whitelist).
    pub namespaces: Vec<String>,
    /// Class local names declared by the active OntologyDescription.
    pub classes: Vec<String>,
    /// Source OD id (e.g. `OD-001`).
    pub source_od: String,
}

/// Store query surface — how to address the orchestration store from
/// inside a step. Derived from the target env's `env_type` via a fixed
/// per-type table.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreQuerySurface {
    /// Surface kind (`local-oxigraph` or `remote-http`).
    pub kind: String,
    /// The literal command or template the worker uses to address it.
    pub query_command: String,
    /// Optional endpoint URL (for remote-http envs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Concrete env capabilities — binaries on PATH, writable paths, allowed
/// HTTP hosts, env variables. Populated from the env's optional
/// `dec:concreteCapabilities` block, falling back to a per-env-type
/// default table (with a warning recorded on the bundle metadata).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvCapabilities {
    /// Binaries the worker may reference as the head of a `shell-command`.
    pub binaries_on_path: Vec<String>,
    /// File-path prefixes the worker may write to in `file-assertion` /
    /// `shell-command`.
    pub writable_paths: Vec<String>,
    /// HTTP hosts the worker may use in `http-request`.
    pub allowed_hosts: Vec<String>,
    /// Environment variable names the worker may reference in `capture`.
    pub environment_variables: Vec<String>,
    /// Pre-seeded artifacts visible to the worker in the env.
    #[serde(default)]
    pub pre_seeded_artifacts: Vec<String>,
}

impl EnvCapabilities {
    /// Build an `EnvCapabilities` from a parsed `ConcreteCapabilities`
    /// blank node read from the env's Turtle file.
    #[must_use]
    pub fn from_concrete(c: &ConcreteCapabilities) -> Self {
        Self {
            binaries_on_path: c.binaries_on_path.clone(),
            writable_paths: c.writable_paths.clone(),
            allowed_hosts: c.allowed_hosts.clone(),
            environment_variables: c.environment_variables.clone(),
            pre_seeded_artifacts: c.pre_seeded_artifacts.clone(),
        }
    }
}

/// Side-channel parse of the optional `dec:concreteCapabilities` block.
/// The core `VerificationBench` type does not carry this block —
/// it's an FT-102 extension; rather than touching the core struct, the
/// enrichment module reads it directly off the env's Turtle file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConcreteCapabilities {
    /// Binaries the worker may invoke as the head of a `shell-command` step.
    pub binaries_on_path: Vec<String>,
    /// File-path prefixes the worker may write to.
    pub writable_paths: Vec<String>,
    /// HTTP hosts the worker may use in `http-request` steps.
    pub allowed_hosts: Vec<String>,
    /// Environment variable names the worker may reference in `capture`.
    pub environment_variables: Vec<String>,
    /// Pre-seeded artifacts available in the env.
    pub pre_seeded_artifacts: Vec<String>,
}

/// One exemplar graph record. `pattern_name` + `rationale` are the
/// fields the worker reads for pattern-matching guidance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExemplarRecord {
    /// Short EX-id (e.g. `EX-001`).
    pub id: String,
    /// IRI of the underlying VG.
    pub exemplar_of: String,
    /// Short pattern name slug (e.g. `store-init-then-sparql`).
    pub pattern_name: String,
    /// Long-form rationale.
    pub rationale: String,
    /// Safety class this exemplar applies to.
    pub safety_class: String,
}

/// Replay-determinism metadata. Records per-artifact content hashes and
/// any warnings raised during assembly (e.g. env-type fallback).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleMetadata {
    /// Per-artifact content-hashes for the catalog artifacts pulled into
    /// the bundle. Keys are short ids (`CR-001`, `OD-001`, `EX-002`).
    pub catalog_hashes: Vec<CatalogHashEntry>,
    /// Soft-warnings raised during assembly. Empty when nothing
    /// notable happened.
    pub warnings: Vec<String>,
}

/// One catalog-hash entry (artifact short id → content hash).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogHashEntry {
    /// Short id of the catalog artifact (`CR-NNN` / `OD-NNN` / `EX-NNN`).
    pub id: String,
    /// SHA-256 hex digest over the artifact's canonical serialisation.
    pub content_hash: String,
}

/// Build the enrichment block by SPARQL-querying the catalog and
/// reading the env's optional `dec:concreteCapabilities` block.
///
/// Returns `Error::CatalogIncomplete` when a mandatory field
/// (`cli_surface`, `ontology_vocabulary`) has zero artifacts in the
/// catalog. `exemplar_graphs` is advisory — an empty list yields a
/// warning, not an error.
pub fn assemble_enrichment(
    store: &Store,
    env: Option<&VerificationBench>,
    concrete: Option<&ConcreteCapabilities>,
    dec_version: &str,
) -> Result<EnrichmentFields, HandlerError> {
    let mut metadata = BundleMetadata::default();

    let cli_surface = query_cli_surface(store, dec_version, &mut metadata)?;
    let ontology_vocabulary = query_ontology_vocabulary(store, &mut metadata)?;

    if strict_catalog_required() {
        let mut missing: Vec<String> = Vec::new();
        if cli_surface.commands.is_empty() {
            missing.push("cli_surface".to_string());
        }
        if ontology_vocabulary.namespace.is_empty() {
            missing.push("ontology_vocabulary".to_string());
        }
        if !missing.is_empty() {
            return Err(catalog_incomplete_error(missing));
        }
    } else {
        // Lenient mode (default): empty catalog ⇒ best-effort bundle
        // with warnings recorded on the metadata block.
        if cli_surface.commands.is_empty() {
            metadata.warnings.push(
                "catalog is empty for cli_surface; worker will proceed without dec subcommand constraints"
                    .to_string(),
            );
        }
        if ontology_vocabulary.namespace.is_empty() {
            metadata.warnings.push(
                "catalog is empty for ontology_vocabulary; worker will proceed without namespace constraints"
                    .to_string(),
            );
        }
    }

    let store_query_surface = derive_store_query_surface(env);
    let catalog_populated =
        !cli_surface.commands.is_empty() || !ontology_vocabulary.namespace.is_empty();
    let env_capabilities = derive_env_capabilities(env, concrete, catalog_populated, &mut metadata);
    let exemplar_graphs = query_exemplars(store, env, &mut metadata)?;

    Ok(EnrichmentFields {
        cli_surface,
        ontology_vocabulary,
        store_query_surface,
        env_capabilities,
        exemplar_graphs,
        bundle_metadata: metadata,
    })
}

use std::cell::Cell;

thread_local! {
    /// Test-only thread-local override for strict-catalog mode. When
    /// `Some(true)`, [`strict_catalog_required`] returns true regardless
    /// of the env var. Production paths leave this `None` and the env
    /// var (`DEC_VERIFY_REQUIRE_CATALOG`) is the sole signal.
    static STRICT_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// RAII guard returned by [`set_strict_override`] — restores the previous
/// override on drop.
pub struct StrictOverrideGuard {
    prev: Option<bool>,
}

impl Drop for StrictOverrideGuard {
    fn drop(&mut self) {
        let prev = self.prev;
        STRICT_OVERRIDE.with(|cell| cell.set(prev));
    }
}

/// Install a thread-local strict-mode override. Used by tests to flip
/// `strict_catalog_required` without touching the process-wide env var.
#[must_use]
pub fn set_strict_override(value: bool) -> StrictOverrideGuard {
    let prev = STRICT_OVERRIDE.with(|cell| {
        let prev = cell.get();
        cell.set(Some(value));
        prev
    });
    StrictOverrideGuard { prev }
}

/// Strict mode toggles `CatalogIncomplete` from a warning into an error.
/// Defaults off so the FT-049-era tests (with empty catalogs) keep
/// working; the FT-102 catalog-incomplete TC sets this via either the
/// thread-local override ([`set_strict_override`]) or the
/// `DEC_VERIFY_REQUIRE_CATALOG=1` env var.
#[must_use]
pub fn strict_catalog_required() -> bool {
    if let Some(v) = STRICT_OVERRIDE.with(|cell| cell.get()) {
        return v;
    }
    std::env::var("DEC_VERIFY_REQUIRE_CATALOG")
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Build the `CatalogIncomplete` error variant. Uses the
/// `HandlerError::Internal` carrier with a stable `CatalogIncomplete:`
/// prefix so renderers and tests can match either the code-level
/// `Internal` discriminant or the prefixed substring.
pub(crate) fn catalog_incomplete_error(missing_fields: Vec<String>) -> HandlerError {
    let detail = format!(
        "CatalogIncomplete: missing_fields = [{joined}]; \
         seed via `python3 scripts/bootstrap_catalog.py` or author \
         CapabilityReference/OntologyDescription artifacts via the catalog CLI",
        joined = missing_fields.join(", ")
    );
    HandlerError::Internal { detail }
}

fn query_cli_surface(
    store: &Store,
    dec_version: &str,
    metadata: &mut BundleMetadata,
) -> Result<CliSurface, HandlerError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?s ?cmd ?ver WHERE {{ \
             GRAPH <{cat}> {{ \
                 ?s a dec:CapabilityReference ; \
                    dec:command ?cmd ; \
                    dec:capabilityVersion ?ver . \
                 FILTER NOT EXISTS {{ ?s dec:supersededBy ?_ }} \
                 FILTER (str(?ver) = \"{ver}\") \
             }} \
         }} ORDER BY ?s",
        cat = IRI_DEC_GRAPH_CATALOG,
        ver = dec_version,
    );
    let results = store
        .query(q.as_str())
        .map_err(|e| HandlerError::Internal {
            detail: format!("enrichment: query_cli_surface SPARQL failed: {e}"),
        })?;
    let mut commands: Vec<CliCommand> = Vec::new();
    if let QueryResults::Solutions(sols) = results {
        for sol in sols.flatten() {
            let s = sol.get("s");
            let cmd = sol.get("cmd");
            let ver = sol.get("ver");
            if let (
                Some(Term::NamedNode(subject)),
                Some(Term::Literal(cmd_lit)),
                Some(Term::Literal(ver_lit)),
            ) = (s, cmd, ver)
            {
                let cr_id = short_id_from_iri(subject.as_str(), "https://decision-cli.dev/ns/cr/");
                metadata.catalog_hashes.push(CatalogHashEntry {
                    id: cr_id.clone(),
                    content_hash: content_hash_of(&format!(
                        "{}|{}|{}",
                        subject.as_str(),
                        cmd_lit.value(),
                        ver_lit.value()
                    )),
                });
                commands.push(CliCommand {
                    command: cmd_lit.value().to_string(),
                    capability_version: ver_lit.value().to_string(),
                    source_cr: cr_id,
                });
            }
        }
    }
    let dec_subcommands: Vec<String> = commands
        .iter()
        .filter_map(|c| {
            if c.command.starts_with("dec ") {
                Some(c.command.clone())
            } else {
                None
            }
        })
        .collect();
    Ok(CliSurface {
        commands,
        dec_subcommands,
        capability_version: dec_version.to_string(),
        init_templates: bundled_init_templates(),
    })
}

/// Enumerates the bundled value-stream templates the running `dec`
/// binary ships. The list is the file stem of every
/// `streams/<name>.ttl` baked into the binary via
/// `core::bundled::stream_template_list`. Kept here so the
/// enrichment writer doesn't reach across module boundaries — the
/// list is small (currently just `engineering-development`).
fn bundled_init_templates() -> Vec<String> {
    crate::core::bundled::known_template_names()
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn query_ontology_vocabulary(
    store: &Store,
    metadata: &mut BundleMetadata,
) -> Result<OntologyVocabulary, HandlerError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?s ?ns ?prefix ?body WHERE {{ \
             GRAPH <{cat}> {{ \
                 ?s a dec:OntologyDescription ; \
                    dec:namespace ?ns ; \
                    dec:prefix ?prefix ; \
                    dec:ontologyBody ?body . \
                 FILTER NOT EXISTS {{ ?s dec:supersededBy ?_ }} \
             }} \
         }} LIMIT 1",
        cat = IRI_DEC_GRAPH_CATALOG,
    );
    let results = store
        .query(q.as_str())
        .map_err(|e| HandlerError::Internal {
            detail: format!("enrichment: query_ontology_vocabulary SPARQL failed: {e}"),
        })?;
    let mut vocab = OntologyVocabulary::default();
    if let QueryResults::Solutions(sols) = results {
        for sol in sols.flatten() {
            let s = sol.get("s");
            let ns = sol.get("ns");
            let prefix = sol.get("prefix");
            let body = sol.get("body");
            if let (
                Some(Term::NamedNode(subject)),
                Some(Term::Literal(ns_lit)),
                Some(Term::Literal(prefix_lit)),
                Some(Term::Literal(body_lit)),
            ) = (s, ns, prefix, body)
            {
                let od_id = short_id_from_iri(subject.as_str(), "https://decision-cli.dev/ns/od/");
                metadata.catalog_hashes.push(CatalogHashEntry {
                    id: od_id.clone(),
                    content_hash: content_hash_of(&format!(
                        "{}|{}|{}",
                        subject.as_str(),
                        ns_lit.value(),
                        body_lit.value()
                    )),
                });
                vocab.namespace = ns_lit.value().to_string();
                vocab.prefix = prefix_lit.value().to_string();
                vocab.source_od = od_id;
                vocab.classes = extract_classes_from_body(body_lit.value());
                vocab.namespaces = std::iter::once(vocab.namespace.clone())
                    .chain(w3c_whitelist().iter().map(|s| s.to_string()))
                    .collect();
                break;
            }
        }
    }
    Ok(vocab)
}

/// Extract class local-names from the JSON body of an OntologyDescription.
/// Tolerates several shapes (`classes: [{local_name: "..."}]`, plain string lists).
fn extract_classes_from_body(body: &str) -> Vec<String> {
    let parsed: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let Some(arr) = parsed.get("classes").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(arr.len());
    for entry in arr {
        if let Some(s) = entry.as_str() {
            out.push(s.to_string());
            continue;
        }
        if let Some(name) = entry
            .get("local_name")
            .or_else(|| entry.get("name"))
            .and_then(|v| v.as_str())
        {
            out.push(name.to_string());
        }
    }
    out
}

/// W3C namespaces the chokepoint validator whitelists per ADR-066.
#[must_use]
pub fn w3c_whitelist() -> &'static [&'static str] {
    &[
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "http://www.w3.org/2000/01/rdf-schema#",
        "http://www.w3.org/2001/XMLSchema#",
        "http://www.w3.org/2002/07/owl#",
        "http://www.w3.org/ns/prov#",
        "http://purl.org/dc/terms/",
    ]
}

/// Per-env-type default for the store query surface. Lives in code per
/// ADR-066 §The validator's whitelist (the env-type table is structural).
fn derive_store_query_surface(env: Option<&VerificationBench>) -> StoreQuerySurface {
    let Some(env) = env else {
        return StoreQuerySurface {
            kind: "local-oxigraph".to_string(),
            query_command: "dec sparql query --store".to_string(),
            endpoint: None,
        };
    };
    if env.is_remote() {
        StoreQuerySurface {
            kind: "remote-http".to_string(),
            query_command: "curl -sf -X POST".to_string(),
            endpoint: env.endpoint.clone(),
        }
    } else {
        StoreQuerySurface {
            kind: "local-oxigraph".to_string(),
            query_command: "dec sparql query --store".to_string(),
            endpoint: None,
        }
    }
}

/// Resolve `EnvCapabilities` from the env + an optional side-channel
/// `ConcreteCapabilities` block. Falls back to a per-env-type default
/// **only when the catalog is populated** — for empty-catalog dispatches
/// (pre-FT-102, or operator-bootstrapping flows), leave `EnvCapabilities`
/// empty so the validator stays lenient.
fn derive_env_capabilities(
    env: Option<&VerificationBench>,
    concrete: Option<&ConcreteCapabilities>,
    catalog_populated: bool,
    metadata: &mut BundleMetadata,
) -> EnvCapabilities {
    if let Some(c) = concrete {
        return EnvCapabilities::from_concrete(c);
    }
    if !catalog_populated {
        // Lenient: don't synthesise capability constraints when no
        // catalog is in place. The validator's empty-field check then
        // skips membership tests entirely.
        return EnvCapabilities::default();
    }
    let Some(env) = env else {
        return default_env_capabilities_for("ephemeral-tempdir");
    };
    metadata.warnings.push(format!(
        "env {id} has no dec:concreteCapabilities block; using env-type default for {env_type}",
        id = env.id,
        env_type = env.bench_type,
    ));
    default_env_capabilities_for(&env.bench_type)
}

/// Per-env-type defaults shipped alongside the assembler. New env types
/// must extend this table (and the SHACL shape) atomically.
#[must_use]
pub fn default_env_capabilities_for(bench_type: &str) -> EnvCapabilities {
    match bench_type {
        "ephemeral-tempdir" => EnvCapabilities {
            // Expanded from the slice-1 minimal set (`dec`, `bash`, `jq`)
            // to include the dev-tool basics every TC runner needs. The
            // ephemeral-tempdir bench is the default for orchestrator
            // self-tests; every TC runner the operator-facing
            // `runner: cargo-test | pytest | bash | …` set names must
            // appear here, otherwise the validator rejects every step
            // that lifts those runners (see TC-208…TC-252 for the
            // typical inputs). A future ADR should make this list
            // bench-extensible from the TTL so operators can tighten
            // per-bench without a code change.
            binaries_on_path: vec![
                "dec".to_string(),
                "bash".to_string(),
                "sh".to_string(),
                "jq".to_string(),
                "cargo".to_string(),
                "python".to_string(),
                "python3".to_string(),
                "pytest".to_string(),
                "uv".to_string(),
                "git".to_string(),
                "grep".to_string(),
                "find".to_string(),
                "make".to_string(),
                "ls".to_string(),
                "diff".to_string(),
                "sed".to_string(),
                "awk".to_string(),
                "cat".to_string(),
            ],
            writable_paths: vec!["$DEC_VERIFY_TMP".to_string(), "./".to_string()],
            allowed_hosts: Vec::new(),
            environment_variables: vec![
                "DEC_VERIFY_TMP".to_string(),
                "DEC_PROJECT_ROOT".to_string(),
                "PATH".to_string(),
                "HOME".to_string(),
            ],
            pre_seeded_artifacts: Vec::new(),
        },
        "remote-http" => EnvCapabilities {
            binaries_on_path: vec!["curl".to_string(), "jq".to_string()],
            writable_paths: vec!["$DEC_VERIFY_TMP".to_string()],
            allowed_hosts: Vec::new(),
            environment_variables: vec!["PATH".to_string(), "HOME".to_string()],
            pre_seeded_artifacts: Vec::new(),
        },
        _ => EnvCapabilities {
            binaries_on_path: vec!["dec".to_string(), "bash".to_string()],
            writable_paths: vec!["./".to_string()],
            allowed_hosts: Vec::new(),
            environment_variables: vec!["PATH".to_string()],
            pre_seeded_artifacts: Vec::new(),
        },
    }
}

fn query_exemplars(
    store: &Store,
    env: Option<&VerificationBench>,
    metadata: &mut BundleMetadata,
) -> Result<Vec<ExemplarRecord>, HandlerError> {
    let safety_class = env
        .map(|e| e.safety_class.as_str().to_string())
        .unwrap_or_default();
    if safety_class.is_empty() {
        metadata
            .warnings
            .push("no env supplied; skipping exemplar lookup".to_string());
        return Ok(Vec::new());
    }
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?s ?vg ?pattern ?rationale WHERE {{ \
             GRAPH <{cat}> {{ \
                 ?s a dec:ExemplarGraph ; \
                    dec:exemplarOf ?vg ; \
                    dec:appliesToSafetyClass ?sc ; \
                    dec:patternName ?pattern ; \
                    dec:rationale ?rationale . \
                 FILTER NOT EXISTS {{ ?s dec:supersededBy ?_ }} \
                 FILTER (str(?sc) = \"{sc}\") \
             }} \
         }} ORDER BY ?s LIMIT 5",
        cat = IRI_DEC_GRAPH_CATALOG,
        sc = safety_class,
    );
    let results = store
        .query(q.as_str())
        .map_err(|e| HandlerError::Internal {
            detail: format!("enrichment: query_exemplars SPARQL failed: {e}"),
        })?;
    let mut out: Vec<ExemplarRecord> = Vec::new();
    if let QueryResults::Solutions(sols) = results {
        for sol in sols.flatten() {
            let s = sol.get("s");
            let vg = sol.get("vg");
            let pattern = sol.get("pattern");
            let rationale = sol.get("rationale");
            if let (
                Some(Term::NamedNode(subject)),
                Some(Term::NamedNode(vg_node)),
                Some(Term::Literal(pattern_lit)),
                Some(Term::Literal(rationale_lit)),
            ) = (s, vg, pattern, rationale)
            {
                let ex_id = short_id_from_iri(subject.as_str(), "https://decision-cli.dev/ns/ex/");
                metadata.catalog_hashes.push(CatalogHashEntry {
                    id: ex_id.clone(),
                    content_hash: content_hash_of(&format!(
                        "{}|{}|{}|{}",
                        subject.as_str(),
                        vg_node.as_str(),
                        pattern_lit.value(),
                        rationale_lit.value()
                    )),
                });
                out.push(ExemplarRecord {
                    id: ex_id,
                    exemplar_of: vg_node.as_str().to_string(),
                    pattern_name: pattern_lit.value().to_string(),
                    rationale: rationale_lit.value().to_string(),
                    safety_class: safety_class.clone(),
                });
            }
        }
    }
    if out.is_empty() {
        metadata.warnings.push(format!(
            "no exemplar graphs found for safety_class {safety_class}"
        ));
    }
    Ok(out)
}

/// Read the optional `dec:concreteCapabilities` block from an env's
/// Turtle file. Returns `Ok(None)` when the file has no such block.
pub fn read_concrete_capabilities_from_turtle(
    path: &Path,
) -> Result<Option<ConcreteCapabilities>, HandlerError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| HandlerError::Internal {
        detail: format!("enrichment: reading env file {p}: {e}", p = path.display()),
    })?;
    let store = Store::new().map_err(|e| HandlerError::Internal {
        detail: format!("enrichment: temp store: {e}"),
    })?;
    let staging = NamedNode::new_unchecked("urn:decision-cli:env-concrete-staging");
    let parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(staging.clone()));
    store
        .load_from_reader(parser, bytes.as_slice())
        .map_err(|e| HandlerError::Internal {
            detail: format!("enrichment: parse env Turtle: {e}"),
        })?;
    // Locate the env subject + its concreteCapabilities blank node.
    // Quote the staging graph IRI so the query restricts to it.
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?cc WHERE {{ \
             GRAPH <{g}> {{ \
                 ?env a dec:VerificationBench ; \
                      dec:concreteCapabilities ?cc \
             }} \
         }}",
        g = staging.as_str()
    );
    let results = store
        .query(q.as_str())
        .map_err(|e| HandlerError::Internal {
            detail: format!("enrichment: locate concreteCapabilities: {e}"),
        })?;
    let QueryResults::Solutions(sols) = results else {
        return Ok(None);
    };
    let mut subject_term: Option<Term> = None;
    for sol in sols.flatten() {
        if let Some(t) = sol.get("cc") {
            subject_term = Some(t.clone());
            break;
        }
    }
    let Some(subject) = subject_term else {
        return Ok(None);
    };
    let mut out = ConcreteCapabilities::default();
    out.binaries_on_path = walk_list(
        &store,
        &subject,
        "https://decision-cli.dev/ns#binariesOnPath",
    );
    out.writable_paths = walk_list(
        &store,
        &subject,
        "https://decision-cli.dev/ns#writablePaths",
    );
    out.allowed_hosts = walk_list(&store, &subject, "https://decision-cli.dev/ns#allowedHosts");
    out.environment_variables = walk_list(
        &store,
        &subject,
        "https://decision-cli.dev/ns#environmentVariables",
    );
    out.pre_seeded_artifacts = walk_list(
        &store,
        &subject,
        "https://decision-cli.dev/ns#preSeededArtifacts",
    );
    Ok(Some(out))
}

/// Walk an rdf:List rooted at `subject` via `predicate`. Returns an empty
/// vec when the predicate is missing or the list collapses to `rdf:nil`.
fn walk_list(store: &Store, subject: &Term, predicate: &str) -> Vec<String> {
    let pred = NamedNode::new_unchecked(predicate);
    let subj_ref = match subject {
        Term::NamedNode(n) => oxigraph::model::Subject::NamedNode(n.clone()),
        Term::BlankNode(b) => oxigraph::model::Subject::BlankNode(b.clone()),
        _ => return Vec::new(),
    };
    let mut head_term: Option<Term> = None;
    for quad in store
        .quads_for_pattern(Some(subj_ref.as_ref()), Some(pred.as_ref()), None, None)
        .filter_map(Result::ok)
    {
        head_term = Some(quad.object);
        break;
    }
    let Some(mut current) = head_term else {
        return Vec::new();
    };
    let rdf_first = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#first");
    let rdf_rest = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest");
    let rdf_nil = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
    let mut out: Vec<String> = Vec::new();
    let mut depth = 0;
    loop {
        if depth > 64 {
            // Defensive cap; rdf:Lists in env Turtle are short.
            break;
        }
        if matches!(&current, Term::NamedNode(n) if n.as_str() == rdf_nil) {
            break;
        }
        let cur_subj = match &current {
            Term::NamedNode(n) => oxigraph::model::Subject::NamedNode(n.clone()),
            Term::BlankNode(b) => oxigraph::model::Subject::BlankNode(b.clone()),
            _ => break,
        };
        // first
        for quad in store
            .quads_for_pattern(
                Some(cur_subj.as_ref()),
                Some(rdf_first.as_ref()),
                None,
                None,
            )
            .filter_map(Result::ok)
        {
            if let Term::Literal(lit) = quad.object {
                out.push(lit.value().to_string());
            }
            break;
        }
        // rest
        let mut next: Option<Term> = None;
        for quad in store
            .quads_for_pattern(Some(cur_subj.as_ref()), Some(rdf_rest.as_ref()), None, None)
            .filter_map(Result::ok)
        {
            next = Some(quad.object);
            break;
        }
        match next {
            Some(n) => current = n,
            None => break,
        }
        depth += 1;
    }
    out
}

fn short_id_from_iri(iri: &str, prefix: &str) -> String {
    iri.strip_prefix(prefix).unwrap_or(iri).to_string()
}

fn content_hash_of(payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_env_capabilities_for_ephemeral_includes_dec_and_bash() {
        let caps = default_env_capabilities_for("ephemeral-tempdir");
        assert!(caps.binaries_on_path.iter().any(|b| b == "dec"));
        assert!(caps.binaries_on_path.iter().any(|b| b == "bash"));
    }

    #[test]
    fn w3c_whitelist_contains_prov_and_xsd() {
        let list = w3c_whitelist();
        assert!(list.contains(&"http://www.w3.org/ns/prov#"));
        assert!(list.contains(&"http://www.w3.org/2001/XMLSchema#"));
    }

    #[test]
    fn extract_classes_handles_objects_and_strings() {
        let body = r#"{"classes":[{"local_name":"VerificationGraph"},"VerificationStep",{"name":"Session"}]}"#;
        let classes = extract_classes_from_body(body);
        assert_eq!(
            classes,
            vec![
                "VerificationGraph".to_string(),
                "VerificationStep".to_string(),
                "Session".to_string(),
            ]
        );
    }

    #[test]
    fn catalog_incomplete_error_carries_field_names() {
        let err = catalog_incomplete_error(vec!["cli_surface".to_string()]);
        let s = format!("{err}");
        assert!(s.contains("CatalogIncomplete"));
        assert!(s.contains("cli_surface"));
    }
}
