//! `dec verify bench new` — single-handler implementation (FT-038 / ADR-029).
//!
//! One handler, two surfaces: the clap-driven CLI in
//! `crates/decision-cli/src/cli/verify.rs` and the MCP tool descriptor
//! in [`tool_descriptor`] both construct an [`BenchNewRequest`] and route
//! it through [`run`]. Business logic lives exclusively here per ADR-029
//! §Single-handler discipline.
//!
//! Behaviour mirrors FT-038 §Behaviour: validate inputs, mint or accept
//! a `BNCH-NNN` id, build the `VerificationBench`, commit through
//! `StreamWriter` (SHACL chokepoint), write the canonical Turtle file.

pub mod mint;
pub mod persist;
mod validate;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::handler::{Error as HandlerError, Request, Response};
use crate::core::mcp::{ToolDescriptor, ToolHandler};
use crate::core::ontology::verification_bench::{
    to_canonical_turtle, SafetyClass, VerificationBench,
};
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::vocab::{
    verify_bench_graph, IRI_DEC_BENCH_PREFIX, IRI_DEC_GRAPH_VERIFY_BENCH,
    IRI_DEC_VERIFICATION_BENCH, SAFETY_ISOLATED, SAFETY_PRODUCTION_READONLY,
    SAFETY_SHARED_NON_DESTRUCTIVE,
};
use crate::core::StreamWriter;

/// MCP tool name — referenced by `cli::verify::run` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_bench_new";

/// Structured request the single handler consumes (both surfaces produce this).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchNewRequest {
    /// Caller-supplied id (e.g. `BNCH-007`). `None` mints the next free id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Bench type tag — e.g. `ephemeral-tempdir`, `remote-http`.
    pub bench_type: String,
    /// Safety class — one of the three controlled values.
    pub safety_class: String,
    /// Ordered list of operation tokens permitted in the environment.
    pub allowed_ops: Vec<String>,
    /// Optional `dec:setup` shell snippet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// Optional `dec:teardown` shell snippet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teardown: Option<String>,
    /// Required iff `env_type` matches `remote-*`; forbidden otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Optional repo-relative path to a fixture tree materialised before
    /// steps execute (FT-053 / ADR-032). `None` when the bench carries no
    /// fixture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_source: Option<String>,
    /// Working directory the handler runs against. Surface adapters set
    /// this; the MCP server overrides any client-supplied value with its
    /// own launch workdir before invoking the handler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// Structured response — surfaced verbatim by MCP, rendered as text on CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchNewResponse {
    /// Minted (or echoed) `BNCH-NNN[-...]` id.
    pub id: String,
    /// Absolute path to the persisted `.dec/verify/bench/<id>.ttl` file.
    pub path: PathBuf,
}

/// Parse the structured `Request` envelope into [`BenchNewRequest`].
///
/// MCP delivers a `Value::Object` matching the input schema; the CLI
/// builds the same object from clap args. Both paths land here.
pub fn parse_request(req: &Request) -> Result<BenchNewRequest, HandlerError> {
    let mut parsed: BenchNewRequest = serde_json::from_value(req.arguments.clone()).map_err(|e| {
        HandlerError::InvalidArgument {
            field: "arguments".to_string(),
            detail: format!("malformed dec_verify_bench_new arguments: {e}"),
        }
    })?;
    if parsed.workdir.is_none() {
        parsed.workdir = std::env::current_dir().ok();
    }
    Ok(parsed)
}

/// MCP tool descriptor — registered by `features::mcp::build_registry`.
#[must_use]
pub fn tool_descriptor() -> ToolDescriptor {
    ToolDescriptor::new(
        TOOL_NAME,
        "Create a new dec:VerificationBench artifact (FT-038 / ADR-028).",
        input_schema(),
        tool_handler(),
    )
    .with_output_schema(output_schema())
}

/// MCP handler closure — runs the single handler and renders the response.
fn tool_handler() -> ToolHandler {
    Arc::new(|req: Request| {
        let parsed = parse_request(&req)?;
        let outcome = run(&parsed)?;
        let summary = format!(
            "created bench {id} at {path}",
            id = outcome.id,
            path = outcome.path.display()
        );
        Ok(Response::with_summary(
            json!({
                "id": outcome.id,
                "path": outcome.path,
            }),
            summary,
        ))
    })
}

/// JSON Schema for the MCP tool's structured output.
fn output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "path"],
        "properties": {
            "id": { "type": "string" },
            "path": { "type": "string" },
        },
    })
}

/// JSON Schema describing the MCP tool's input arguments.
#[must_use]
pub fn input_schema() -> Value {
    json!({
        "type": "object",
        "required": ["bench_type", "safety_class", "allowed_ops"],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string" },
            "bench_type": { "type": "string", "minLength": 1 },
            "safety_class": {
                "type": "string",
                "enum": [SAFETY_ISOLATED, SAFETY_SHARED_NON_DESTRUCTIVE, SAFETY_PRODUCTION_READONLY],
            },
            "allowed_ops": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 },
                "minItems": 1,
            },
            "setup": { "type": "string" },
            "teardown": { "type": "string" },
            "endpoint": { "type": "string" },
            "fixture_source": { "type": "string", "minLength": 1 },
            "workdir": { "type": "string" },
        },
    })
}

/// Single handler — both CLI and MCP surfaces invoke this.
pub fn run(req: &BenchNewRequest) -> Result<BenchNewResponse, HandlerError> {
    let workdir = req
        .workdir
        .as_deref()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    validate::pre_validate(req)?;
    let bench_dir = workdir.join(".dec").join("verify").join("bench");
    let id = resolve_id(req, workdir, &bench_dir)?;
    let bench = build_bench(&id, req)?;
    write_through_writer(workdir, &bench)?;
    let path = persist::write_bench_file(&bench_dir, &id, &bench)?;
    Ok(BenchNewResponse {
        id,
        path: path.canonicalize().unwrap_or(path),
    })
}

/// Resolve the bench id, consulting both the on-disk `.ttl` directory and
/// the orchestration store. Per TC-094 / FT-038 §Invariants, duplicate
/// detection must consult the **store** (the authoritative source) and
/// not just the on-disk `.ttl` file. Disk/store drift (a missing `.ttl`
/// with the artifact still in the store — e.g. after `rm`, a gitignore
/// mismatch, or test cleanup) used to slip past file-only detection and
/// silently appended a second `dec:allowedOps` head, corrupting the bench.
fn resolve_id(req: &BenchNewRequest, workdir: &Path, bench_dir: &Path) -> Result<String, HandlerError> {
    match &req.id {
        Some(id) => {
            let file_dup = mint::id_exists(bench_dir, id).map_err(io_err)?;
            let store_dup = bench_exists_in_store(workdir, id)?;
            if file_dup || store_dup {
                Err(HandlerError::DuplicateId { id: id.clone() })
            } else {
                Ok(id.clone())
            }
        }
        None => mint::mint_next_id(bench_dir).map_err(io_err),
    }
}

/// True iff the orchestration store already contains a
/// `dec:VerificationBench` whose IRI is `<bench-prefix><id>`. Used
/// alongside the on-disk check so drift between `.dec/verify/bench/` and
/// the store cannot produce a silent multi-headed bench (TC-094).
fn bench_exists_in_store(workdir: &Path, id: &str) -> Result<bool, HandlerError> {
    let dump_path = orchestration_dump_path(workdir);
    if !dump_path.exists() {
        return Ok(false);
    }
    let store = load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })?;
    bench_iri_exists(&store, id)
}

/// SPARQL ASK helper: true iff `<bench-prefix><id>` is typed
/// `dec:VerificationBench` in the verify-bench named graph.
fn bench_iri_exists(store: &Store, id: &str) -> Result<bool, HandlerError> {
    let q = format!(
        "ASK {{ GRAPH <{graph}> {{ <{prefix}{id}> a <{cls}> }} }}",
        graph = IRI_DEC_GRAPH_VERIFY_BENCH,
        prefix = IRI_DEC_BENCH_PREFIX,
        cls = IRI_DEC_VERIFICATION_BENCH,
    );
    match store
        .query(q.as_str())
        .map_err(|e| HandlerError::Internal {
            detail: format!("bench-exists ASK: {e}"),
        })? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Err(HandlerError::Internal {
            detail: "bench-exists ASK returned non-boolean result".to_string(),
        }),
    }
}

fn build_bench(id: &str, req: &BenchNewRequest) -> Result<VerificationBench, HandlerError> {
    let safety_class =
        SafetyClass::parse(&req.safety_class).ok_or_else(|| HandlerError::InvalidArgument {
            field: "safety_class".to_string(),
            detail: format!("unknown safety class {got:?}", got = req.safety_class),
        })?;
    Ok(VerificationBench {
        id: id.to_string(),
        bench_type: req.bench_type.clone(),
        setup: req.setup.clone().filter(|s| !s.is_empty()),
        teardown: req.teardown.clone().filter(|s| !s.is_empty()),
        allowed_ops: req.allowed_ops.clone(),
        safety_class,
        endpoint: req.endpoint.clone().filter(|s| !s.is_empty()),
        fixture_source: req.fixture_source.clone().filter(|s| !s.is_empty()),
    })
}

/// Project the bench into the persisted orchestration store through
/// `StreamWriter` (SHACL chokepoint) and persist the dump back to disk.
fn write_through_writer(workdir: &Path, bench: &VerificationBench) -> Result<(), HandlerError> {
    let scope = ActiveScope::load(workdir).map_err(|e| HandlerError::Internal {
        detail: format!("loading active scope: {e}"),
    })?;
    let dump_path = orchestration_dump_path(workdir);
    let store = load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })?;
    let store = Arc::new(store);
    let stream_iri = NamedNode::new(&scope.stream_iri).map_err(|e| HandlerError::Internal {
        detail: format!("active stream iri {iri}: {e}", iri = scope.stream_iri),
    })?;
    let writer =
        StreamWriter::open(Arc::clone(&store), stream_iri).map_err(|e| HandlerError::Internal {
            detail: format!("opening stream writer: {e}"),
        })?;
    let quads = bench.to_quads(verify_bench_graph());
    writer.commit(Mutation::insert(quads)).map_err(|e| {
        let msg = format!("{e:#}");
        if msg.contains("SHACL violation") {
            HandlerError::SchemaViolation { detail: msg }
        } else {
            HandlerError::Internal { detail: msg }
        }
    })?;
    persist_store(&store, &dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("persisting store: {e}"),
    })?;
    Ok(())
}

fn io_err(e: std::io::Error) -> HandlerError {
    HandlerError::Internal {
        detail: format!("io: {e}"),
    }
}

/// Canonical Turtle that would be written for a request. Exposed for
/// the parity test in TC-060 — both surfaces produce byte-identical
/// Turtle once the id is held constant.
#[must_use]
pub fn canonical_turtle(bench: &VerificationBench) -> String {
    to_canonical_turtle(bench)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_descriptor_has_canonical_name() {
        let d = tool_descriptor();
        assert_eq!(d.name, TOOL_NAME);
    }

    #[test]
    fn request_round_trips_through_json() {
        let req = BenchNewRequest {
            id: Some("BNCH-042".to_string()),
            bench_type: "ephemeral-tempdir".to_string(),
            safety_class: SAFETY_ISOLATED.to_string(),
            allowed_ops: vec!["shell".to_string()],
            setup: Some("echo hi".to_string()),
            teardown: None,
            endpoint: None,
            fixture_source: None,
            workdir: None,
        };
        let v = serde_json::to_value(&req).expect("ser");
        let back: BenchNewRequest = serde_json::from_value(v).expect("de");
        assert_eq!(req, back);
    }

    #[test]
    fn input_schema_declares_required_fields() {
        let s = input_schema();
        let required = s
            .get("required")
            .and_then(|v| v.as_array())
            .expect("required array");
        let names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(names.contains(&"bench_type"));
        assert!(names.contains(&"safety_class"));
        assert!(names.contains(&"allowed_ops"));
    }

    /// TC-094: a bench that exists only in the store (file removed)
    /// must still be detected by [`bench_iri_exists`]. Empty store
    /// returns false; a typed subject returns true.
    #[test]
    fn bench_iri_exists_consults_store_not_disk() {
        let store = Store::new().expect("store");
        // Empty store: no bench known.
        assert!(!bench_iri_exists(&store, "BNCH-T").expect("empty"));

        // Seed a `dec:VerificationBench` typed subject directly
        // into the verify-bench named graph — mirrors what
        // `StreamWriter::commit(bench_to_quads)` would persist.
        let iri = NamedNode::new(format!("{IRI_DEC_BENCH_PREFIX}BNCH-T")).expect("iri");
        let rdf_type =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").expect("rdf:type");
        let cls = NamedNode::new(IRI_DEC_VERIFICATION_BENCH).expect("cls");
        let graph = NamedNode::new(IRI_DEC_GRAPH_VERIFY_BENCH).expect("graph");
        let quad = oxigraph::model::Quad::new(
            iri,
            rdf_type,
            cls,
            oxigraph::model::GraphName::NamedNode(graph),
        );
        store
            .transaction(|mut tx| tx.insert(quad.as_ref()).map(|_| ()))
            .expect("insert");
        assert!(bench_iri_exists(&store, "BNCH-T").expect("seeded"));
        assert!(!bench_iri_exists(&store, "BNCH-U").expect("other id"));
    }
}
