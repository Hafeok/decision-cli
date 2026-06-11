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

use crate::core::ontology::catalog::{
    CapabilityReference, ExemplarGraph, OntologyDescription, SafetyClassTag,
};
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
    /// Count of `dec:ExemplarGraph` artifacts inserted in this run.
    pub exemplars_written: usize,
    /// Count of exemplars skipped because their IRI already existed.
    pub exemplars_skipped: usize,
}

/// Seed the FT-101 catalog in `workdir`'s orchestration store.
///
/// Idempotent: existing CRs are detected by IRI presence in the catalog
/// graph and skipped. The single canonical OntologyDescription is
/// inserted only when no `dec:OntologyDescription` exists yet.
pub fn seed_ft101_catalog(workdir: &Path) -> Result<SeedReport, SeedError> {
    seed_ft101_catalog_with(workdir, false)
}

/// Variant of [`seed_ft101_catalog`] that overwrites existing artifacts
/// when `force` is true (FT-107.D — used after baked-in CR bodies have
/// been corrected to match the live `dec --help` ground truth).
pub fn seed_ft101_catalog_with(workdir: &Path, force: bool) -> Result<SeedReport, SeedError> {
    use oxigraph::model::Subject;

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
        let existed = iri_present(&store, &cr.iri());
        if existed && !force {
            report.capabilities_skipped += 1;
            continue;
        }
        if existed {
            let removes: Vec<oxigraph::model::Quad> = store
                .quads_for_pattern(
                    Some(Subject::NamedNode(cr.iri()).as_ref()),
                    None,
                    None,
                    None,
                )
                .filter_map(Result::ok)
                .collect();
            if !removes.is_empty() {
                writer
                    .commit(Mutation {
                        removes,
                        ..Mutation::default()
                    })
                    .map_err(|e| SeedError::Commit(format!("CR {} retract: {e:#}", cr.id)))?;
            }
        }
        let quads = cr.to_quads();
        writer
            .commit(Mutation::insert(quads))
            .map_err(|e| SeedError::Commit(format!("CR {}: {e:#}", cr.id)))?;
        report.capabilities_written += 1;
    }

    let od = canonical_ontology_description();
    let od_exists = iri_present(&store, &od.iri());
    if od_exists && force {
        let removes: Vec<oxigraph::model::Quad> = store
            .quads_for_pattern(
                Some(Subject::NamedNode(od.iri()).as_ref()),
                None,
                None,
                None,
            )
            .filter_map(Result::ok)
            .collect();
        if !removes.is_empty() {
            writer
                .commit(Mutation {
                    removes,
                    ..Mutation::default()
                })
                .map_err(|e| SeedError::Commit(format!("OD {} retract: {e:#}", od.id)))?;
        }
    }
    if !ontology_description_present(&store) || (od_exists && force) {
        let quads = od.to_quads();
        writer
            .commit(Mutation::insert(quads))
            .map_err(|e| SeedError::Commit(format!("OD {}: {e:#}", od.id)))?;
        report.ontology_written = true;
    }

    // Exemplar graphs: pattern-templates the worker may copy from.
    for ex in exemplar_graphs() {
        let existed = iri_present(&store, &ex.iri());
        if existed && !force {
            report.exemplars_skipped += 1;
            continue;
        }
        if existed {
            let removes: Vec<oxigraph::model::Quad> = store
                .quads_for_pattern(
                    Some(Subject::NamedNode(ex.iri()).as_ref()),
                    None,
                    None,
                    None,
                )
                .filter_map(Result::ok)
                .collect();
            if !removes.is_empty() {
                writer
                    .commit(Mutation {
                        removes,
                        ..Mutation::default()
                    })
                    .map_err(|e| SeedError::Commit(format!("EX {} retract: {e:#}", ex.id)))?;
            }
        }
        let quads = ex.to_quads();
        writer
            .commit(Mutation::insert(quads))
            .map_err(|e| SeedError::Commit(format!("EX {}: {e:#}", ex.id)))?;
        report.exemplars_written += 1;
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
pub fn deactivate_role_binding(workdir: &Path, binding_iri: &str) -> Result<bool, SeedError> {
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
    let active_pred = NamedNodeRef::new_unchecked(crate::core::vocab::IRI_DEC_ROLE_BINDING_ACTIVE);

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
    let rdf_type = NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
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

/// Hand-curated exemplar graphs pulled from VGs that produced an
/// approved VerificationGraphResult. FT-101 §"3-5 per env type"; we
/// seed three patterns each for the isolated safety class. The worker
/// pattern-matches against these instead of inventing structures from
/// scratch.
fn exemplar_graphs() -> Vec<ExemplarGraph> {
    use oxigraph::model::NamedNode;
    vec![
        ExemplarGraph {
            id: "EX-INIT-DOCTOR".to_string(),
            exemplar_of: NamedNode::new_unchecked("https://decision-cli.dev/ns/graph/VG-005"),
            applies_to_safety_class: SafetyClassTag::Isolated,
            pattern_name: "init-then-introspect".to_string(),
            rationale: "Canonical opening for any verification graph that needs an initialised \
                 orchestration store. Step 0 runs `dec init --template engineering-development` \
                 (the only way `dec init` succeeds without a Turtle definition file in scope); \
                 Step 1 then introspects via a stable subcommand like `dec doctor --format json`. \
                 Use this pattern whenever a TC requires verifying anything about the post-init \
                 store state."
                .to_string(),
            based_on_approved_result: NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/result/VGR-012",
            ),
            supersedes: None,
        },
        ExemplarGraph {
            id: "EX-CLI-INSPECT".to_string(),
            exemplar_of: NamedNode::new_unchecked("https://decision-cli.dev/ns/graph/VG-017"),
            applies_to_safety_class: SafetyClassTag::Isolated,
            pattern_name: "cli-help-inspection".to_string(),
            rationale:
                "Pattern for verifying that a CLI surface exposes the expected subcommand or \
                 flag. Each step runs `dec <subcmd> --help` and asserts exit 0. Cheap, fast, \
                 and the right tool when the TC's claim is about command availability rather \
                 than runtime behaviour. Don't use for behavioural verification — for that, run \
                 the subcommand and assert observable effects."
                    .to_string(),
            based_on_approved_result: NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/result/VGR-024",
            ),
            supersedes: None,
        },
        ExemplarGraph {
            id: "EX-FILESYSTEM-GREP".to_string(),
            exemplar_of: NamedNode::new_unchecked("https://decision-cli.dev/ns/graph/VG-008"),
            applies_to_safety_class: SafetyClassTag::Isolated,
            pattern_name: "filesystem-negative-grep".to_string(),
            rationale: "Pattern for asserting absence: search the repo for a forbidden symbol and \
                 assert exit 1 (no matches). Form: `find <dir> -name '*.py' -exec grep -l \
                 '<symbol>' {} \\;` with `dec:expectExitCode 1`. Use when the TC claims a \
                 cleanup/removal happened (e.g. \"no hardcoded model names remain in \
                 workers/\")."
                .to_string(),
            based_on_approved_result: NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/result/VGR-015",
            ),
            supersedes: None,
        },
    ]
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
  "required_flags_one_of": ["--from", "--template"],
  "flags": [
    {"name": "--from", "value_kind": "path", "description": "Path to a ValueStream TTL file. Mutually exclusive with --template; ONE is required."},
    {"name": "--template", "value_kind": "string", "description": "Bundled template id (e.g. 'engineering-development'). Mutually exclusive with --from; ONE is required."}
  ],
  "positionals": [],
  "exit_codes": [
    {"code": 0, "meaning": "store initialised"},
    {"code": 1, "meaning": "validation/IO failure"},
    {"code": 2, "meaning": "neither --from nor --template was supplied, OR the working directory is already initialised"}
  ],
  "observable_effects": [
    {"kind": "file_written", "path_pattern": ".dec/store/orchestration.nq"},
    {"kind": "file_written", "path_pattern": ".dec/definition.ttl"},
    {"kind": "file_written", "path_pattern": ".dec/init-metadata.json"},
    {"kind": "file_written", "path_pattern": ".dec/verify/env/BNCH-001-ephemeral-cli.ttl"}
  ],
  "common_invocations": [
    "dec init --template engineering-development",
    "dec init --from ./streams/decision-cli-development.ttl"
  ],
  "notes": "Does NOT create '.dec/config.toml'. Refuses to overwrite an existing '.dec/' tree (exits 2)."
}"#,
    },
    BakedCapability {
        id: "CR-STATUS",
        command: "dec status",
        body: r#"{
  "command": "dec status",
  "synopsis": "Report the active value stream's bootstrap provenance.",
  "flags": [],
  "positionals": [],
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
  "synopsis": "Liveness check. Runs outside an initialised working tree.",
  "flags": [],
  "positionals": [],
  "exit_codes": [{"code": 0, "meaning": "healthy"}]
}"#,
    },
    BakedCapability {
        id: "CR-DOCTOR",
        command: "dec doctor",
        body: r#"{
  "command": "dec doctor",
  "synopsis": "Worker preflight audit; checks that every bound worker binary resolves on $PATH and reports its version.",
  "flags": [],
  "positionals": [],
  "exit_codes": [
    {"code": 0, "meaning": "all workers resolve"},
    {"code": 1, "meaning": "one or more workers missing"}
  ]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-GRAPH-NEW",
        command: "dec verify graph new",
        body: r#"{
  "command": "dec verify graph new",
  "synopsis": "Author a new dec:VerificationGraph artifact.",
  "required_flags": ["--verifies", "--environment"],
  "flags": [
    {"name": "--verifies", "value_kind": "string", "required": true, "description": "FT-NNN or TC-NNN id this graph verifies."},
    {"name": "--environment", "value_kind": "string", "required": true, "description": "Env id (e.g. BNCH-001-ephemeral-cli)."},
    {"name": "--id", "value_kind": "string", "required": false, "description": "Caller-supplied VG-NNN id; omitted → mints the next free VG-NNN."}
  ],
  "positionals": [],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-STEP-ADD",
        command: "dec verify step add",
        body: r#"{
  "command": "dec verify step add",
  "synopsis": "Append a typed step to an existing VerificationGraph.",
  "required_flags": ["--type"],
  "flags": [
    {"name": "--type", "value_kind": "string", "required": true, "description": "Step kind: shell-command | sparql-assertion | file-assertion | http-request | wait-for | capture."},
    {"name": "--field", "value_kind": "key=value", "required": false, "repeatable": true, "description": "Per-kind field, e.g. --field command='dec status'. Repeatable."},
    {"name": "--provides-evidence-for", "value_kind": "string", "required": false, "repeatable": true, "description": "TC-NNN short id this step provides evidence for. Repeatable."}
  ],
  "positionals": [{"name": "GRAPH_ID", "value_kind": "string", "required": true, "description": "VG-NNN id of the target graph (positional, not a --graph flag)."}],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-GRAPH-GENERATE",
        command: "dec verify graph generate",
        body: r#"{
  "command": "dec verify graph generate",
  "synopsis": "Propose a graph for a feature in an environment (worker-driven).",
  "required_flags": ["--environment"],
  "flags": [
    {"name": "--environment", "value_kind": "string", "required": true, "description": "Env id (e.g. BNCH-001-ephemeral-cli)."},
    {"name": "--accept", "value_kind": "boolean", "required": false, "description": "Persist without prompting."},
    {"name": "--print-only", "value_kind": "boolean", "required": false, "description": "Show the proposal, never persist."}
  ],
  "positionals": [{"name": "FEATURE_ID", "value_kind": "string", "required": true, "description": "FT-NNN id (positional)."}],
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
  "positionals": [{"name": "GRAPH_ID", "value_kind": "string", "required": true, "description": "VG-NNN id (positional)."}],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-FEATURE",
        command: "dec verify feature",
        body: r#"{
  "command": "dec verify feature",
  "synopsis": "Verify a feature by running every covering VerificationGraph and aggregating verdicts.",
  "flags": [
    {"name": "--environment", "value_kind": "string", "required": false, "description": "Filter to one environment (BNCH-NNN[-suffix])."},
    {"name": "--no-feedback", "value_kind": "boolean", "required": false, "description": "Skip Feedback emission."},
    {"name": "--include-stale", "value_kind": "boolean", "required": false, "description": "Consider VGRs older than the freshness window."},
    {"name": "--dry-run", "value_kind": "boolean", "required": false, "description": "Enumerate which graphs would run; do not execute."}
  ],
  "positionals": [{"name": "FEATURE_ID", "value_kind": "string", "required": true, "description": "FT-NNN id (positional). Note: there is NO --feature-id flag."}],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-VERIFY-ENV-NEW",
        command: "dec verify bench new",
        body: r#"{
  "command": "dec verify bench new",
  "synopsis": "Author a new dec:VerificationBench artifact.",
  "required_flags": ["--type", "--safety-class", "--allowed-ops"],
  "flags": [
    {"name": "--type", "value_kind": "string", "required": true, "description": "Env kind (e.g. ephemeral-tempdir | repo-path | remote-http)."},
    {"name": "--safety-class", "value_kind": "string", "required": true, "description": "isolated | shared-non-destructive | production-readonly."},
    {"name": "--allowed-ops", "value_kind": "csv", "required": true, "description": "Comma-separated operation tokens (e.g. 'shell,filesystem')."},
    {"name": "--id", "value_kind": "string", "required": false, "description": "Caller-supplied BNCH-NNN id."},
    {"name": "--setup", "value_kind": "string", "required": false, "description": "Optional setup shell snippet."},
    {"name": "--teardown", "value_kind": "string", "required": false, "description": "Optional teardown shell snippet."}
  ],
  "positionals": [],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-FEEDBACK-LIST",
        command: "dec feedback list",
        body: r#"{
  "command": "dec feedback list",
  "synopsis": "List open feedback grouped by class and target role.",
  "flags": [
    {"name": "--state", "value_kind": "string", "required": false, "description": "Restrict to a lifecycle state (produced | routed | received | addressed)."},
    {"name": "--class", "value_kind": "string", "required": false, "description": "Restrict to a feedback class (gap | defect | contradiction | ...)."},
    {"name": "--target", "value_kind": "string", "required": false, "description": "Restrict to a target role id."}
  ],
  "positionals": [],
  "exit_codes": [{"code": 0, "meaning": "ok"}]
}"#,
    },
    BakedCapability {
        id: "CR-EVENTS-TAIL",
        command: "dec events tail",
        body: r#"{
  "command": "dec events tail",
  "synopsis": "Stream events live from the SSE endpoint of a running `dec` daemon.",
  "flags": [
    {"name": "--url", "value_kind": "url", "required": false, "description": "Override the SSE endpoint."}
  ],
  "positionals": [],
  "exit_codes": [{"code": 0, "meaning": "ok"}]
}"#,
    },
    BakedCapability {
        id: "CR-IMPLEMENT",
        command: "dec implement",
        body: r#"{
  "command": "dec implement",
  "synopsis": "Implement a feature end-to-end via the code-writer worker.",
  "flags": [
    {"name": "--workspace", "value_kind": "path", "required": false, "description": "Workspace dir the worker writes into."},
    {"name": "--product-root", "value_kind": "path", "required": false, "description": "Override .product/ root."},
    {"name": "--worker", "value_kind": "string", "required": false, "description": "Override the worker command (default `code-writer`)."},
    {"name": "--bundle-depth", "value_kind": "integer", "required": false, "description": "Depth passed to `product context`."},
    {"name": "--waive-coverage", "value_kind": "string", "required": false, "description": "Override the chain-integrity gate with a rationale (>= 16 chars)."}
  ],
  "positionals": [{"name": "FEATURE_ID", "value_kind": "string", "required": true, "description": "FT-NNN id (positional)."}],
  "exit_codes": [{"code": 0, "meaning": "ok"}, {"code": 1, "meaning": "error"}]
}"#,
    },
    BakedCapability {
        id: "CR-PREFLIGHT",
        command: "dec preflight",
        body: r#"{
  "command": "dec preflight",
  "synopsis": "Feature-coverage report from the internal product-cli graph projection.",
  "flags": [],
  "positionals": [{"name": "FEATURE_ID", "value_kind": "string", "required": true, "description": "FT-NNN id (positional)."}],
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
    {"local_name": "VerificationBench", "iri": "https://decision-cli.dev/ns#VerificationBench",
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
       {"local_name": "environment", "range": "dec:VerificationBench"},
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
