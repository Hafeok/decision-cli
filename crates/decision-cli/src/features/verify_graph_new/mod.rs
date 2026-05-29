//! `dec verify graph new` — single-handler implementation (FT-041 / ADR-029).
//!
//! One handler, two surfaces: the clap-driven CLI in
//! `crates/decision-cli/src/cli/verify.rs` and the MCP tool descriptor
//! in [`tool_descriptor`] both construct a [`GraphNewRequest`] and route
//! it through [`run`]. Business logic lives exclusively here per ADR-029
//! §Single-handler discipline.
//!
//! Behaviour mirrors FT-041 §Behaviour: resolve `verifies` / `environment`
//! references, mint or accept a `VG-NNN` id, build the empty
//! `VerificationGraph` value, commit through `StreamWriter` (SHACL
//! chokepoint), write the canonical Turtle file.

pub mod mint;
pub mod persist;
mod resolve;

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
use crate::core::ontology::verification_graph::{ArtifactRef, VerificationGraph};
use crate::core::scope::ActiveScope;
use crate::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use crate::core::vocab::{
    verify_graph_named_graph, IRI_DEC_GRAPH_VERIFY_GRAPH, IRI_DEC_VERIFICATION_GRAPH,
    IRI_DEC_VERIFY_GRAPH_PREFIX,
};
use crate::core::StreamWriter;

/// MCP tool name — referenced by `cli::verify` for the parity TC.
pub const TOOL_NAME: &str = "dec_verify_graph_new";

/// Structured request the single handler consumes (both surfaces produce this).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNewRequest {
    /// Caller-supplied id (e.g. `VG-007`). `None` mints the next free id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Feature or TC reference (`FT-NNN` / `TC-NNN`).
    pub verifies: String,
    /// Environment reference (`BNCH-NNN[-suffix]`).
    pub environment: String,
    /// Working directory the handler runs against. Surface adapters set
    /// this; the MCP server overrides any client-supplied value with its
    /// own launch workdir before invoking the handler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
}

/// Structured response — surfaced verbatim by MCP, rendered as text on CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNewResponse {
    /// Minted (or echoed) `VG-NNN[-…]` id.
    pub id: String,
    /// Absolute path to the persisted `.dec/verify/graph/<id>.ttl` file.
    pub path: PathBuf,
}

/// Parse the structured `Request` envelope into [`GraphNewRequest`].
///
/// MCP delivers a `Value::Object` matching the input schema; the CLI
/// builds the same object from clap args. Both paths land here.
pub fn parse_request(req: &Request) -> Result<GraphNewRequest, HandlerError> {
    let mut parsed: GraphNewRequest =
        serde_json::from_value(req.arguments.clone()).map_err(|e| {
            HandlerError::InvalidArgument {
                field: "arguments".to_string(),
                detail: format!("malformed dec_verify_graph_new arguments: {e}"),
            }
        })?;
    if parsed.workdir.is_none() {
        parsed.workdir = std::env::current_dir().ok();
    }
    Ok(parsed)
}

/// MCP tool descriptor — registered by `cli::mcp::build_production_registry`.
#[must_use]
pub fn tool_descriptor() -> ToolDescriptor {
    ToolDescriptor::new(
        TOOL_NAME,
        "Create a new dec:VerificationGraph artifact (FT-041 / ADR-028).",
        input_schema(),
        build_tool_handler(),
    )
    .with_output_schema(output_schema())
}

fn build_tool_handler() -> ToolHandler {
    Arc::new(|req: Request| {
        let parsed = parse_request(&req)?;
        let outcome = run(&parsed)?;
        let summary = format!(
            "created graph {id} at {path}",
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
        "required": ["verifies", "environment"],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string" },
            "verifies": { "type": "string", "minLength": 1 },
            "environment": { "type": "string", "minLength": 1 },
            "workdir": { "type": "string" },
        },
    })
}

/// Single handler — both CLI and MCP surfaces invoke this.
pub fn run(req: &GraphNewRequest) -> Result<GraphNewResponse, HandlerError> {
    let workdir = req
        .workdir
        .as_deref()
        .ok_or_else(|| HandlerError::InvalidArgument {
            field: "workdir".to_string(),
            detail: "no working directory available; run from a `dec init`-bootstrapped tree"
                .to_string(),
        })?;
    if let Some(id) = &req.id {
        validate_id_format(id)?;
    }
    // Resolve reference fields *before* any I/O: caller-facing errors
    // (DanglingRef, InvalidArgument) must surface ahead of disk writes.
    let verifies_iri = resolve::resolve_verifies(workdir, &req.verifies)?;
    let environment_iri = resolve::resolve_environment(workdir, &req.environment)?;

    let graph_dir = workdir.join(".dec").join("verify").join("graph");
    let id = resolve_id(req, workdir, &graph_dir)?;
    let graph = VerificationGraph::new(&id, ArtifactRef(verifies_iri), environment_iri, Vec::new());
    write_through_writer(workdir, &graph)?;
    let path = persist::write_graph_file(&graph_dir, &id, &graph)?;
    Ok(GraphNewResponse {
        id,
        path: path.canonicalize().unwrap_or(path),
    })
}

/// Resolve the graph id, consulting both the on-disk `.ttl` directory and
/// the orchestration store. Per TC-095 / FT-041 §Invariants, duplicate
/// detection must consult the **store** (the authoritative source) and
/// not just the on-disk `.ttl` file. Disk/store drift (a missing `.ttl`
/// with the artifact still in the store — e.g. after `rm`, a gitignore
/// mismatch, or test cleanup) used to slip past file-only detection and
/// silently appended a second `dec:environment` binding (and other
/// triples) into the graph, producing a multi-headed projection.
fn resolve_id(
    req: &GraphNewRequest,
    workdir: &Path,
    graph_dir: &Path,
) -> Result<String, HandlerError> {
    match &req.id {
        Some(id) => {
            let file_dup = mint::id_exists(graph_dir, id).map_err(io_err)?;
            let store_dup = graph_exists_in_store(workdir, id)?;
            if file_dup || store_dup {
                Err(HandlerError::DuplicateId { id: id.clone() })
            } else {
                Ok(id.clone())
            }
        }
        None => mint::mint_next_id(graph_dir).map_err(io_err),
    }
}

/// True iff the orchestration store already contains a
/// `dec:VerificationGraph` whose IRI is `<graph-prefix><id>`. Used
/// alongside the on-disk check so drift between `.dec/verify/graph/` and
/// the store cannot produce a silent multi-headed graph (TC-095).
fn graph_exists_in_store(workdir: &Path, id: &str) -> Result<bool, HandlerError> {
    let dump_path = orchestration_dump_path(workdir);
    if !dump_path.exists() {
        return Ok(false);
    }
    let store = load_store_from_dump(&dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })?;
    graph_iri_exists(&store, id)
}

/// SPARQL ASK helper: true iff `<graph-prefix><id>` is typed
/// `dec:VerificationGraph` in the verify-graph named graph.
fn graph_iri_exists(store: &Store, id: &str) -> Result<bool, HandlerError> {
    let q = format!(
        "ASK {{ GRAPH <{graph}> {{ <{prefix}{id}> a <{cls}> }} }}",
        graph = IRI_DEC_GRAPH_VERIFY_GRAPH,
        prefix = IRI_DEC_VERIFY_GRAPH_PREFIX,
        cls = IRI_DEC_VERIFICATION_GRAPH,
    );
    match store.query(q.as_str()).map_err(|e| HandlerError::Internal {
        detail: format!("graph-exists ASK: {e}"),
    })? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Err(HandlerError::Internal {
            detail: "graph-exists ASK returned non-boolean result".to_string(),
        }),
    }
}

fn validate_id_format(id: &str) -> Result<(), HandlerError> {
    if !id.starts_with("VG-") {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("graph id must start with 'VG-', got {id:?}"),
        });
    }
    if id.len() < 4 {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("graph id {id:?} is too short (expected VG-NNN[-suffix])"),
        });
    }
    let tail = &id[3..];
    if !tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(HandlerError::InvalidArgument {
            field: "id".to_string(),
            detail: format!("graph id {id:?} must match VG-NNN[-suffix]"),
        });
    }
    Ok(())
}

/// Project the graph into the persisted orchestration store through
/// `StreamWriter` (SHACL chokepoint) and persist the dump back to disk.
fn write_through_writer(workdir: &Path, graph: &VerificationGraph) -> Result<(), HandlerError> {
    let scope = ActiveScope::load(workdir).map_err(|e| HandlerError::Internal {
        detail: format!("loading active scope: {e}"),
    })?;
    let dump_path = orchestration_dump_path(workdir);
    let store = open_store(&dump_path)?;
    let writer = open_writer(Arc::clone(&store), &scope.stream_iri)?;
    let quads = graph.to_quads(verify_graph_named_graph());
    writer
        .commit(Mutation::insert(quads))
        .map_err(commit_to_handler_error)?;
    persist_store(&store, &dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("persisting store: {e}"),
    })?;
    Ok(())
}

fn open_store(dump_path: &Path) -> Result<Arc<oxigraph::store::Store>, HandlerError> {
    let store = load_store_from_dump(dump_path).map_err(|e| HandlerError::Internal {
        detail: format!("loading orchestration store: {e}"),
    })?;
    Ok(Arc::new(store))
}

fn open_writer(
    store: Arc<oxigraph::store::Store>,
    stream_iri: &str,
) -> Result<StreamWriter, HandlerError> {
    let iri = NamedNode::new(stream_iri).map_err(|e| HandlerError::Internal {
        detail: format!("active stream iri {stream_iri}: {e}"),
    })?;
    StreamWriter::open(store, iri).map_err(|e| HandlerError::Internal {
        detail: format!("opening stream writer: {e}"),
    })
}

fn commit_to_handler_error<E: std::fmt::Display>(e: E) -> HandlerError {
    let msg = format!("{e:#}");
    if msg.contains("SHACL violation") {
        HandlerError::SchemaViolation { detail: msg }
    } else {
        HandlerError::Internal { detail: msg }
    }
}

fn io_err(e: std::io::Error) -> HandlerError {
    HandlerError::Internal {
        detail: format!("io: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_descriptor_has_canonical_name() {
        assert_eq!(tool_descriptor().name, TOOL_NAME);
    }

    #[test]
    fn request_round_trips_through_json() {
        let req = GraphNewRequest {
            id: Some("VG-042".to_string()),
            verifies: "FT-001".to_string(),
            environment: "BNCH-001-ephemeral-cli".to_string(),
            workdir: None,
        };
        let v = serde_json::to_value(&req).expect("ser");
        let back: GraphNewRequest = serde_json::from_value(v).expect("de");
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
        assert!(names.contains(&"verifies"));
        assert!(names.contains(&"environment"));
    }

    #[test]
    fn validate_id_format_accepts_plain_vg_nnn() {
        validate_id_format("VG-007").expect("VG-007 ok");
    }

    #[test]
    fn validate_id_format_rejects_missing_prefix() {
        let err = validate_id_format("foo").expect_err("must fail");
        match err {
            HandlerError::InvalidArgument { field, .. } => assert_eq!(field, "id"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// TC-095: a graph that exists only in the store (file removed)
    /// must still be detected by [`graph_iri_exists`]. Empty store
    /// returns false; a typed subject returns true.
    #[test]
    fn graph_iri_exists_consults_store_not_disk() {
        let store = Store::new().expect("store");
        // Empty store: no graph known.
        assert!(!graph_iri_exists(&store, "VG-T").expect("empty"));

        // Seed a `dec:VerificationGraph` typed subject directly into
        // the verify-graph named graph — mirrors what
        // `StreamWriter::commit(graph_to_quads)` would persist.
        let iri =
            NamedNode::new(format!("{IRI_DEC_VERIFY_GRAPH_PREFIX}VG-T")).expect("iri");
        let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
            .expect("rdf:type");
        let cls = NamedNode::new(IRI_DEC_VERIFICATION_GRAPH).expect("cls");
        let graph = NamedNode::new(IRI_DEC_GRAPH_VERIFY_GRAPH).expect("graph");
        let quad = oxigraph::model::Quad::new(
            iri,
            rdf_type,
            cls,
            oxigraph::model::GraphName::NamedNode(graph),
        );
        store
            .transaction(|mut tx| tx.insert(quad.as_ref()).map(|_| ()))
            .expect("insert");
        assert!(graph_iri_exists(&store, "VG-T").expect("seeded"));
        assert!(!graph_iri_exists(&store, "VG-U").expect("other id"));
    }
}
