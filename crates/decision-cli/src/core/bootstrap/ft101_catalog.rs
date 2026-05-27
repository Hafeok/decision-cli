//! FT-107.A — pragmatic bootstrap of the FT-101 catalog (`CapabilityReference`
//! + `OntologyDescription`) so the verify-graph-author bundle assembler's
//! enrichment fields stop arriving empty and the worker has ground truth
//! for `dec` flags / namespace.
//!
//! FT-101 promised `dec catalog capability new` / `dec catalog ontology new`
//! CLI verbs, but those weren't wired (the spec status says complete; the
//! CLI surface doesn't reflect it). This module is the minimal seeder
//! that closes that gap for the live worker-dispatch loop: it inserts a
//! curated set of `CapabilityReference` artifacts (one per major `dec`
//! subcommand the worker may invoke) and a single `OntologyDescription`
//! covering the `dec:` namespace, persisted through the existing
//! `StreamWriter` chokepoint.
//!
//! The seeder is idempotent: re-running it does not duplicate artifacts.
//! Existing CRs (matched by IRI) are skipped.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use oxigraph::model::NamedNode;
use thiserror::Error;

use crate::core::ontology::catalog::{CapabilityReference, OntologyDescription};
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::stream_writer::StreamWriter;
use oxi_events::Mutation;

/// Errors surfaced by [`seed_ft101_catalog`].
#[derive(Debug, Error)]
pub enum SeedError {
    /// The orchestration store at `<workdir>/.dec/store/orchestration.nq`
    /// could not be opened.
    #[error("orchestration store: {0}")]
    Store(String),
    /// Active stream resolution failed.
    #[error("active scope: {0}")]
    Scope(String),
    /// SHACL or write-side validation refused a CR / OD insertion.
    #[error("commit: {0}")]
    Commit(String),
}

/// Summary of one seeder run.
#[derive(Debug, Default)]
pub struct SeedReport {
    /// CRs whose IRI was already in the store; skipped.
    pub capabilities_skipped: usize,
    /// CRs newly inserted in this run.
    pub capabilities_written: usize,
    /// Whether the canonical OntologyDescription was new (true) or
    /// already present (false).
    pub ontology_written: bool,
}

/// Seed the FT-101 catalog in `workdir`'s orchestration store.
///
/// Idempotent: existing CRs are detected by IRI presence in the catalog
/// graph and skipped. The single canonical OntologyDescription is
/// inserted only when no `dec:OntologyDescription` exists yet.
pub fn seed_ft101_catalog(workdir: &Path) -> Result<SeedReport, SeedError> {
    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)
        .map_err(|e| SeedError::Store(format!("loading {}: {e:#}", dump.display())))?;
    let store = Arc::new(store);
    let scope = ActiveScope::load(workdir).map_err(|e| SeedError::Scope(format!("{e}")))?;
    let stream_iri = NamedNode::new(&scope.stream_iri)
        .map_err(|e| SeedError::Scope(format!("active stream iri: {e}")))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .map_err(|e| SeedError::Commit(format!("opening writer: {e:#}")))?;

    let mut report = SeedReport::default();

    for cr in capability_references() {
        if iri_present(&store, &cr.iri()) {
            report.capabilities_skipped += 1;
            continue;
        }
        let quads = cr.to_quads();
        writer
            .commit(Mutation::insert(quads))
            .map_err(|e| SeedError::Commit(format!("CR {}: {e:#}", cr.id)))?;
        report.capabilities_written += 1;
    }

    if !ontology_description_present(&store) {
        let od = canonical_ontology_description();
        let quads = od.to_quads();
        writer
            .commit(Mutation::insert(quads))
            .map_err(|e| SeedError::Commit(format!("OD {}: {e:#}", od.id)))?;
        report.ontology_written = true;
    }

    persist_store(&store, &dump)
        .map_err(|e| SeedError::Store(format!("persisting {}: {e:#}", dump.display())))?;

    Ok(report)
}

/// FT-107 follow-up helper: flip `dec:active "true" → "false"` on a
/// role-binding IRI through the StreamWriter chokepoint, persist the
/// store, and return whether any quads were rewritten.
///
/// Use case: after `dec _bootstrap-catalog` lands a bumped role-binding
/// (v2 alongside v1), the prior v1 is left active and the uniqueness
/// invariant in `core::ontology::role_binding::read::active_for_role`
/// trips with "N active role bindings share the same role_id". This
/// helper deactivates the prior version.
pub fn deactivate_role_binding(
    workdir: &Path,
    binding_iri: &str,
) -> Result<bool, SeedError> {
    use oxigraph::model::{Literal, NamedNodeRef, Quad, Subject, Term};

    let dump = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump)
        .map_err(|e| SeedError::Store(format!("loading {}: {e:#}", dump.display())))?;
    let store = Arc::new(store);
    let scope = ActiveScope::load(workdir).map_err(|e| SeedError::Scope(format!("{e}")))?;
    let stream_iri = NamedNode::new(&scope.stream_iri)
        .map_err(|e| SeedError::Scope(format!("active stream iri: {e}")))?;
    let writer = StreamWriter::open(Arc::clone(&store), stream_iri)
        .map_err(|e| SeedError::Commit(format!("opening writer: {e:#}")))?;

    let subject = NamedNode::new(binding_iri)
        .map_err(|e| SeedError::Commit(format!("invalid binding iri: {e}")))?;
    let active_pred = NamedNodeRef::new_unchecked(
        crate::core::vocab::IRI_DEC_ROLE_BINDING_ACTIVE,
    );

    let removes: Vec<Quad> = store
        .quads_for_pattern(
            Some(Subject::NamedNode(subject.clone()).as_ref()),
            Some(active_pred),
            None,
            None,
        )
        .filter_map(Result::ok)
        .filter(|q| match &q.object {
            Term::Literal(lit) => lit.value() == "true",
            _ => false,
        })
        .collect();
    if removes.is_empty() {
        return Ok(false);
    }

    let xsd_boolean =
        oxigraph::model::NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean");
    let inserts: Vec<Quad> = removes
        .iter()
        .map(|q| {
            Quad::new(
                q.subject.clone(),
                q.predicate.clone(),
                Literal::new_typed_literal("false", xsd_boolean.as_ref()),
                q.graph_name.clone(),
            )
        })
        .collect();

    let mutation = Mutation {
        inserts,
        removes,
        ..Mutation::default()
    };
    writer
        .commit(mutation)
        .map_err(|e| SeedError::Commit(format!("deactivate {binding_iri}: {e:#}")))?;
    persist_store(&store, &dump)
        .map_err(|e| SeedError::Store(format!("persisting {}: {e:#}", dump.display())))?;
    Ok(true)
}

fn iri_present(store: &oxigraph::store::Store, iri: &NamedNode) -> bool {
    use oxigraph::model::Subject;
    store
        .quads_for_pattern(
            Some(Subject::NamedNode(iri.clone()).as_ref()),
            None,
            None,
            None,
        )
        .filter_map(Result::ok)
        .next()
        .is_some()
}

fn ontology_description_present(store: &oxigraph::store::Store) -> bool {
    use oxigraph::model::NamedNodeRef;
    use oxigraph::model::Term;
    let cls = NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#OntologyDescription");
    let rdf_type =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    store
        .quads_for_pattern(
            None,
            Some(rdf_type),
            Some(Term::NamedNode(cls.into_owned()).as_ref()),
            None,
        )
        .filter_map(Result::ok)
        .next()
        .is_some()
}

/// The catalog version the seeded artifacts pin themselves to. Must
/// match `DEFAULT_DEC_VERSION` from
/// `verify_graph_generate::enrichment` — the assembler filters CRs by
/// equality on this literal.
const CATALOG_VERSION: &str = "0.3.0";

fn capability_references() -> Vec<CapabilityReference> {
    BAKED_CAPABILITIES
        .iter()
        .map(|baked| CapabilityReference {
            id: baked.id.to_string(),
            command: baked.command.to_string(),
            capability_version: CATALOG_VERSION.to_string(),
            body: baked.body.to_string(),
            supersedes: None,
        })
        .collect()
}

fn canonical_ontology_description() -> OntologyDescription {
    OntologyDescription {
        id: "OD-001".to_string(),
        namespace: "https://decision-cli.dev/ns#".to_string(),
        prefix: "dec".to_string(),
        ontology_version: CATALOG_VERSION.to_string(),
        body: ONTOLOGY_BODY.to_string(),
        supersedes: None,
    }
}

struct BakedCapability {
    id: &'static str,
    command: &'static str,
    body: &'static str,
}

/// Hand-curated capability references for the dec subcommands the
/// verify-graph-author worker most often needs to script. Source: live
/// `dec <command> --help` output from `dec 0.1.0`.
///
/// These are intentionally minimal — flags + synopsis + exit codes. The
/// worker uses them to avoid hallucinating non-existent flags; deeper
/// behavioural docs live in `Implementing_DDD.md`, not the catalog.
const BAKED_CAPABILITIES: &[BakedCapability] = &[
    BakedCapability {
        id: "CR-INIT",
        command: "dec init",
        body: r#"{
  "command": "dec init",
  "synopsis": "Initialise the orchestration store from a ValueStream definition.",
  "flags": [
    {"name": "--from", "value_kind": "path", "required": false, "description": "Path to a ValueStream TTL file."},
    {"name": "--template", "value_kind": "string", "required": false, "description": "Bundled template id (e.g. 'engineering-development')."}
  ],
  "positionals": [],
  "exit_codes": [
    {"code": 0, "meaning": "store initialised"},
    {"code": 1, "meaning": "validation/IO failure"},
    {"code": 2, "meaning": "wrong working dir or stream missing"}
  ],
  "observable_effects": [
    {"kind": "directory_written", "path_pattern": ".dec/store/"},
    {"kind": "file_written", "path_pattern": ".dec/store/orchestration.nq"}
  ]
}"#,
    },
    BakedCapability {
        id: "CR-STATUS",
        command: "dec status",
        body: r#"{
  "command": "dec status",
  "synopsis": "Report the active value stream's bootstrap provenance.",
  "flags": [],
  "exit_codes": [
    {"code": 0, "meaning": "active stream printed"},
    {"code": 2, "meaning": "no init found"}
  ]
}"#,
    },
    BakedCapability {
        id: "CR-HEALTH",
        command: "dec health",
        body: r#"{
  "command": "dec health",
  "synopsis": "Liveness check; works outside an initialised tree.",
  "flags": [],
  "exit_codes": [{"code": 0, "meaning": "healthy"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-GRAPH-NEW",
        command: "dec verify graph new",
        body: r#"{
  "command": "dec verify graph new",
  "synopsis": "Author a new dec:VerificationGraph artifact.",
  "flags": [
    {"name": "--verifies", "value_kind": "string", "required": true, "description": "FT-NNN or TC-NNN id this graph verifies."},
    {"name": "--environment", "value_kind": "string", "required": true, "description": "Env id (e.g. ENV-001-ephemeral-cli)."},
    {"name": "--id", "value_kind": "string", "required": false, "description": "Optional explicit VG-NNN id."}
  ],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-STEP-ADD",
        command: "dec verify step add",
        body: r#"{
  "command": "dec verify step add",
  "synopsis": "Append a step to an existing VerificationGraph.",
  "flags": [
    {"name": "--graph", "value_kind": "string", "required": true, "description": "VG-NNN id."},
    {"name": "--kind", "value_kind": "string", "required": true, "description": "Step kind (shell-command | file-assertion | sparql-assertion | http-request | wait-for | capture)."},
    {"name": "--field", "value_kind": "key=value", "required": false, "repeatable": true, "description": "One key=value per step field."},
    {"name": "--provides-evidence-for", "value_kind": "string", "required": false, "repeatable": true, "description": "TC-NNN id covered by this step."}
  ],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-GRAPH-GENERATE",
        command: "dec verify graph generate",
        body: r#"{
  "command": "dec verify graph generate",
  "synopsis": "Propose a graph for a feature in an environment (worker-driven).",
  "flags": [
    {"name": "--environment", "value_kind": "string", "required": true, "description": "Env id."},
    {"name": "--accept", "value_kind": "boolean", "required": false, "description": "Persist without prompting."},
    {"name": "--print-only", "value_kind": "boolean", "required": false, "description": "Show the proposal, never persist."}
  ],
  "positionals": [{"name": "FEATURE_ID", "value_kind": "string", "description": "FT-NNN id."}],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-GRAPH-RUN",
        command: "dec verify graph run",
        body: r#"{
  "command": "dec verify graph run",
  "synopsis": "Execute a VerificationGraph and persist its VerificationGraphResult.",
  "flags": [
    {"name": "--capture", "value_kind": "name=value", "required": false, "repeatable": true, "description": "Pre-seeded capture binding."},
    {"name": "--no-feedback", "value_kind": "boolean", "required": false, "description": "Skip Feedback emission."},
    {"name": "--keep-tmp", "value_kind": "boolean", "required": false, "description": "Set DEC_KEEP_TMP=1 to preserve ephemeral env tempdirs."}
  ],
  "positionals": [{"name": "GRAPH_ID", "value_kind": "string", "description": "VG-NNN id."}],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-FEATURE",
        command: "dec verify feature",
        body: r#"{
  "command": "dec verify feature",
  "synopsis": "Verify a feature by running every covering VerificationGraph and aggregating verdicts.",
  "positionals": [{"name": "FEATURE_ID", "value_kind": "string", "description": "FT-NNN id."}],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-ENV-NEW",
        command: "dec verify env new",
        body: r#"{
  "command": "dec verify env new",
  "synopsis": "Author a new dec:VerificationEnvironment artifact.",
  "flags": [
    {"name": "--id", "value_kind": "string", "required": false, "description": "Optional explicit ENV-NNN id."},
    {"name": "--type", "value_kind": "string", "required": true, "description": "Env kind (e.g. ephemeral-tempdir | repo-path | remote-http)."},
    {"name": "--safety-class", "value_kind": "string", "required": true, "description": "isolated | shared-non-destructive | production-readonly."},
    {"name": "--allowed-op", "value_kind": "string", "required": false, "repeatable": true, "description": "Allowed op token."}
  ],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-FEEDBACK-LIST",
        command: "dec feedback list",
        body: r#"{
  "command": "dec feedback list",
  "synopsis": "List open feedback grouped by class and target role.",
  "flags": [],
  "exit_codes": [{"code": 0, "meaning": "ok"}]
}"#,
    },
    BakedCapability {
        id: "CR-EVENTS-TAIL",
        command: "dec events tail",
        body: r#"{
  "command": "dec events tail",
  "synopsis": "Subscribe to live events via SSE.",
  "flags": [
    {"name": "--since", "value_kind": "integer", "required": false, "description": "Replay events from a sequence number."}
  ],
  "exit_codes": [{"code": 0, "meaning": "ok"}]
}"#,
    },
    BakedCapability {
        id: "CR-PREFLIGHT",
        command: "dec preflight",
        body: r#"{
  "command": "dec preflight",
  "synopsis": "Feature-coverage report sourced from the internal product-cli graph projection.",
  "flags": [],
  "positionals": [{"name": "FEATURE_ID", "value_kind": "string", "description": "FT-NNN id."}],
  "exit_codes": [{"code": 0, "meaning": "covered"}, {"code": 1, "meaning": "gaps surfaced"}]
}"#,
    },
];

const ONTOLOGY_BODY: &str = r#"{
  "namespace": "https://decision-cli.dev/ns#",
  "prefix": "dec",
  "classes": [
    {"local_name": "Feature", "iri": "https://decision-cli.dev/ns#Feature",
     "predicates": [
       {"local_name": "dependsOn", "range": "dec:Feature"},
       {"local_name": "tests", "range": "dec:TC"},
       {"local_name": "status", "range": "xsd:string"}
     ]},
    {"local_name": "TC", "iri": "https://decision-cli.dev/ns#TC",
     "predicates": [
       {"local_name": "validates", "range": "dec:Feature | dec:ADR"}
     ]},
    {"local_name": "VerificationEnvironment", "iri": "https://decision-cli.dev/ns#VerificationEnvironment",
     "predicates": [
       {"local_name": "envType", "range": "xsd:string"},
       {"local_name": "safetyClass", "range": "xsd:string (isolated | shared-non-destructive | production-readonly)"},
       {"local_name": "allowedOps", "range": "rdf:List of xsd:string"},
       {"local_name": "setup", "range": "xsd:string (bash)"},
       {"local_name": "teardown", "range": "xsd:string (bash)"},
       {"local_name": "endpoint", "range": "xsd:anyURI"},
       {"local_name": "fixtureSource", "range": "xsd:string (repo-relative path)"}
     ]},
    {"local_name": "VerificationGraph", "iri": "https://decision-cli.dev/ns#VerificationGraph",
     "predicates": [
       {"local_name": "verifies", "range": "dec:Feature | dec:TC"},
       {"local_name": "environment", "range": "dec:VerificationEnvironment"},
       {"local_name": "steps", "range": "rdf:List of dec:VerificationStep"}
     ]},
    {"local_name": "VerificationStep", "iri": "https://decision-cli.dev/ns#VerificationStep",
     "predicates": [
       {"local_name": "stepType", "range": "xsd:string (shell-command | file-assertion | sparql-assertion | http-request | wait-for | capture)"},
       {"local_name": "command", "range": "xsd:string"},
       {"local_name": "path", "range": "xsd:string (relative to $dec_workdir)"},
       {"local_name": "target", "range": "xsd:string (relative to $dec_workdir)"},
       {"local_name": "query", "range": "xsd:string (SPARQL)"},
       {"local_name": "expectExitCode", "range": "xsd:integer"},
       {"local_name": "providesEvidenceFor", "range": "dec:TC"}
     ]},
    {"local_name": "VerificationGraphResult", "iri": "https://decision-cli.dev/ns#VerificationGraphResult",
     "predicates": [
       {"local_name": "resultOf", "range": "dec:VerificationGraph"},
       {"local_name": "verdict", "range": "xsd:string (approved | amendment-required | rejected)"},
       {"local_name": "stepTraces", "range": "rdf:List of dec:VerificationStepTrace"}
     ]},
    {"local_name": "Feedback", "iri": "https://decision-cli.dev/ns#Feedback",
     "predicates": [
       {"local_name": "feedbackClass", "range": "xsd:string (gap | defect | contradiction | scope-issue | unimplementable | capability-request)"},
       {"local_name": "targetRole", "range": "xsd:string"},
       {"local_name": "severity", "range": "xsd:string (error | warning | info)"},
       {"local_name": "evidence", "range": "xsd:string"},
       {"local_name": "lifecycleState", "range": "xsd:string (produced | routed | received | addressed | closed | rejected | superseded)"}
     ]}
  ],
  "step_kind_field_summary": {
    "shell-command": ["command (required)", "expect-exit-code (optional, default 0)", "capture-output (optional)"],
    "file-assertion": ["path (required, relative to $dec_workdir)", "expect-hash (optional, sha256 hex)"],
    "sparql-assertion": ["target (required, path to a .nq/.ttl)", "query (required, SPARQL)", "expect-rows (optional)"],
    "http-request": ["method (required)", "url (required)", "expect-status (optional)"],
    "wait-for": ["condition (required, step IRI to poll)", "timeout (required, ISO-8601 duration)"],
    "capture": ["bind-as (required, name to bind prior step's stdout to)"]
  },
  "env_var_contract": {
    "DEC_VERIFY_TMP": "Resolved ephemeral working directory; canonical writable area.",
    "DEC_WORKDIR": "Alias of DEC_VERIFY_TMP (back-compat).",
    "TMPDIR": "Alias of DEC_VERIFY_TMP (back-compat with seed env scripts)."
  }
}"#;

/// Test helper — also used as a sanity check that the embedded JSON
/// payloads round-trip through `serde_json`.
#[cfg(test)]
pub(crate) fn baked_capability_ids() -> Vec<&'static str> {
    BAKED_CAPABILITIES.iter().map(|b| b.id).collect()
}

// Silence the unused-import warning when `anyhow::Context` isn't yet
// pulled in via a `.context()` call (kept on import for future
// extension paths in this module).
#[allow(dead_code)]
fn _force_anyhow_context_use() -> Result<()> {
    let _: Option<&str> = None;
    None::<i32>.context("placeholder").map(|_| ())
}
