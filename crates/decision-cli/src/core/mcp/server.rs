//! JSON-RPC 2.0 over stdio MCP server loop (FT-034 / ADR-029).
//!
//! Run [`serve_stdio`] with a populated [`ToolRegistry`] to start the
//! MCP server. Reads line-delimited JSON-RPC frames from stdin, dispatches
//! against the registry, and writes single-line JSON-RPC responses to
//! stdout. All logging goes to stderr via the `tracing` crate; stdout is
//! reserved for the wire protocol per FT-034 §Behaviour.
//!
//! Shutdown is graceful: EOF on stdin closes the loop after the current
//! handler completes and `serve_stdio` returns `Ok(())` (exit code 0).

use std::io::{BufRead, Write};

use serde_json::{json, Value};
use thiserror::Error;
use tracing::{debug, info, warn};

use super::protocol::{
    Id, RpcErrorFrame, RpcRequest, RpcResponse, ERR_INTERNAL, ERR_INVALID_PARAMS,
    ERR_METHOD_NOT_FOUND, ERR_PARSE, MCP_PROTOCOL_VERSION,
};
use super::registry::ToolRegistry;
use crate::core::handler::{Error as HandlerError, Request};

/// Marker returned by the stdio loop signalling normal EOF shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownSignal;

/// Top-level errors from [`serve_stdio`]. Caller maps to an exit code.
#[derive(Debug, Error)]
pub enum ServeError {
    /// IO failure on stdin/stdout. Per FT-034 §Error handling we log
    /// and exit 1; the caller (`features::mcp::run`) does the mapping.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Run the stdio MCP server against `registry` until EOF.
///
/// On entry, emits the `tracing` line `mcp server ready` at INFO level
/// (TC-053 AC #1). On stdin EOF the function returns; the binary then
/// exits 0 (TC-053 AC #3). Per FT-034 §Behaviour, the function never
/// writes to stdout outside the JSON-RPC envelope.
pub fn serve_stdio(registry: &ToolRegistry) -> Result<ShutdownSignal, ServeError> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stdin_lock = stdin.lock();
    let stdout_lock = stdout.lock();
    serve_stdio_with_io(registry, stdin_lock, stdout_lock)
}

/// Same as [`serve_stdio`] but takes explicit reader/writer handles so
/// integration tests can drive the loop with in-memory streams.
pub fn serve_stdio_with_io<R: BufRead, W: Write>(
    registry: &ToolRegistry,
    mut input: R,
    mut output: W,
) -> Result<ShutdownSignal, ServeError> {
    info!(
        target: "dec_mcp",
        tools = registry.len(),
        "mcp server ready (stdio, protocol {MCP_PROTOCOL_VERSION})"
    );

    let mut line = String::new();
    loop {
        line.clear();
        let bytes = input.read_line(&mut line)?;
        if bytes == 0 {
            info!(target: "dec_mcp", "stdin EOF — shutting down");
            return Ok(ShutdownSignal);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        dispatch_frame(trimmed, registry, &mut output)?;
    }
}

fn dispatch_frame<W: Write>(
    trimmed: &str,
    registry: &ToolRegistry,
    output: &mut W,
) -> std::io::Result<()> {
    match serde_json::from_str::<RpcRequest>(trimmed) {
        Ok(req) => handle_request(req, registry, output),
        Err(err) => {
            warn!(target: "dec_mcp", error = %err, "parse error on incoming frame");
            let frame = RpcErrorFrame::new(
                Id::Null,
                ERR_PARSE,
                "parse error",
                Some(json!({"detail": err.to_string()})),
            );
            write_frame(output, &frame)
        }
    }
}

#[allow(clippy::needless_pass_by_value)] // value form is more ergonomic
fn handle_request<W: Write>(
    req: RpcRequest,
    registry: &ToolRegistry,
    output: &mut W,
) -> std::io::Result<()> {
    debug!(target: "dec_mcp", method = %req.method, "incoming");
    if req.is_notification() {
        // JSON-RPC 2.0: notifications never produce a response.
        return Ok(());
    }
    let id = req.id.clone().unwrap_or(Id::Null);
    match dispatch_method(&req, registry) {
        Ok(result) => write_frame(output, &RpcResponse::ok(id, result)),
        Err(err) => write_frame(output, &err.into_frame(id)),
    }
}

/// Dispatch a request method to its handler. Returns the wire `result`
/// value on success; structured `MethodError` on failure.
fn dispatch_method(req: &RpcRequest, registry: &ToolRegistry) -> Result<Value, MethodError> {
    match req.method.as_str() {
        "initialize" => Ok(initialize_result(registry)),
        "tools/list" => Ok(tools_list_result(registry)),
        "tools/call" => tools_call_result(req.params.as_ref(), registry),
        "ping" => Ok(json!({})),
        other => Err(MethodError::MethodNotFound(other.to_string())),
    }
}

fn initialize_result(registry: &ToolRegistry) -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "dec",
            "version": env!("CARGO_PKG_VERSION"),
            "tools": registry.len(),
        }
    })
}

fn tools_list_result(registry: &ToolRegistry) -> Value {
    let tools: Vec<Value> = registry
        .iter()
        .map(|d| {
            let mut entry = json!({
                "name": d.name,
                "description": d.description,
                "inputSchema": d.input_schema,
            });
            if let Some(out) = &d.output_schema {
                entry["outputSchema"] = out.clone();
            }
            entry
        })
        .collect();
    json!({ "tools": tools })
}

fn tools_call_result(
    params: Option<&Value>,
    registry: &ToolRegistry,
) -> Result<Value, MethodError> {
    let params = params.ok_or_else(|| {
        MethodError::InvalidParams("tools/call requires params with 'name'".to_string())
    })?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| MethodError::InvalidParams("missing string 'name'".to_string()))?
        .to_string();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let descriptor = registry
        .get(&name)
        .ok_or_else(|| MethodError::ToolNotFound(name.clone()))?;
    let request = Request::new(&name, arguments);
    match (descriptor.handler)(request) {
        Ok(response) => Ok(render_tool_result(&response)),
        Err(handler_err) => Ok(render_tool_error(&handler_err)),
    }
}

/// Render a successful handler [`crate::core::handler::Response`] as
/// the MCP `tools/call` result envelope. Always sets `isError: false`.
/// Includes both the structured `data` (as `structuredContent` on the
/// envelope) and a textual rendering in `content`.
fn render_tool_result(response: &crate::core::handler::Response) -> Value {
    let text = response.summary.clone().unwrap_or_else(|| {
        serde_json::to_string(&response.data).unwrap_or_else(|_| "<unserialisable>".to_string())
    });
    json!({
        "content": [
            { "type": "text", "text": text }
        ],
        "structuredContent": response.data,
        "isError": false,
    })
}

/// Render a handler [`HandlerError`] as the MCP `tools/call` result
/// envelope with `isError: true` (per the MCP spec, tool errors do not
/// produce JSON-RPC errors — they produce error-flagged results).
fn render_tool_error(err: &HandlerError) -> Value {
    json!({
        "content": [
            { "type": "text", "text": err.to_string() }
        ],
        "structuredContent": {
            "code": err.code(),
            "error": serde_json::to_value(err).unwrap_or(Value::Null),
        },
        "isError": true,
    })
}

fn write_frame<W: Write, T: serde::Serialize>(output: &mut W, frame: &T) -> std::io::Result<()> {
    let line = serde_json::to_string(frame).unwrap_or_else(|_| {
        "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32603,\"message\":\"serialise failed\"}}"
            .to_string()
    });
    output.write_all(line.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

#[derive(Debug)]
enum MethodError {
    MethodNotFound(String),
    InvalidParams(String),
    ToolNotFound(String),
}

impl MethodError {
    fn into_frame(self, id: Id) -> RpcErrorFrame {
        match self {
            Self::MethodNotFound(m) => RpcErrorFrame::new(
                id,
                ERR_METHOD_NOT_FOUND,
                format!("method not found: {m}"),
                None,
            ),
            Self::InvalidParams(detail) => RpcErrorFrame::new(
                id,
                ERR_INVALID_PARAMS,
                "invalid params",
                Some(json!({"detail": detail})),
            ),
            Self::ToolNotFound(name) => RpcErrorFrame::new(
                id,
                ERR_INTERNAL,
                format!("tool '{name}' not registered"),
                Some(json!({"tool": name})),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::handler::Response;
    use crate::core::mcp::registry::ToolDescriptor;
    use std::io::Cursor;
    use std::sync::Arc;

    fn registry_with_ping() -> ToolRegistry {
        let mut reg = ToolRegistry::new();
        let handler = Arc::new(|req: Request| {
            Ok(Response::with_summary(
                json!({"echo": req.arguments}),
                "pong",
            ))
        });
        reg.register(ToolDescriptor::new(
            "dec_mcp_ping",
            "Echo arguments back",
            json!({"type": "object"}),
            handler,
        ))
        .expect("register ping");
        reg
    }

    fn run_lines(reg: &ToolRegistry, lines: &[&str]) -> Vec<Value> {
        let input = lines
            .iter()
            .map(|l| (*l).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let cursor = Cursor::new(input);
        let mut out: Vec<u8> = Vec::new();
        serve_stdio_with_io(reg, std::io::BufReader::new(cursor), &mut out).expect("serve loop");
        String::from_utf8(out)
            .expect("utf8")
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("json"))
            .collect()
    }

    #[test]
    fn initialize_returns_protocol_version_and_capabilities() {
        let reg = registry_with_ping();
        let resps = run_lines(
            &reg,
            &[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#],
        );
        assert_eq!(resps.len(), 1);
        let result = &resps[0]["result"];
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "dec");
    }

    #[test]
    fn tools_list_returns_registered_tools() {
        let reg = registry_with_ping();
        let resps = run_lines(&reg, &[r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#]);
        let tools = resps[0]["result"]["tools"].as_array().expect("array");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "dec_mcp_ping");
        assert_eq!(tools[0]["description"], "Echo arguments back");
    }

    #[test]
    fn tools_call_routes_through_handler() {
        // TC-053 AC #4: invoking a tool over MCP yields the same
        // structured payload as a direct handler invocation.
        let reg = registry_with_ping();
        let resps = run_lines(
            &reg,
            &[
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"dec_mcp_ping","arguments":{"hi":"there"}}}"#,
            ],
        );
        let result = &resps[0]["result"];
        assert_eq!(result["isError"], false);
        let structured = &result["structuredContent"];
        assert_eq!(structured["echo"]["hi"], "there");
    }

    #[test]
    fn notifications_produce_no_response() {
        let reg = registry_with_ping();
        let resps = run_lines(
            &reg,
            &[
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            ],
        );
        // initialize + tools/list each produce one response; notification
        // produces none, so we expect exactly two frames out.
        assert_eq!(resps.len(), 2, "responses: {resps:?}");
        assert_eq!(resps[0]["id"], 1);
        assert_eq!(resps[1]["id"], 2);
    }

    #[test]
    fn method_not_found_returns_error_frame() {
        let reg = registry_with_ping();
        let resps = run_lines(
            &reg,
            &[r#"{"jsonrpc":"2.0","id":9,"method":"resources/list"}"#],
        );
        assert_eq!(resps[0]["error"]["code"], -32601);
    }

    #[test]
    fn parse_error_returns_error_frame_with_null_id() {
        let reg = registry_with_ping();
        let resps = run_lines(&reg, &["{not json"]);
        assert_eq!(resps[0]["error"]["code"], -32700);
        assert!(resps[0]["id"].is_null());
    }

    #[test]
    fn eof_shuts_down_cleanly() {
        // TC-053 AC #3: closing the input stream (zero-byte read) ends
        // the loop with `Ok(ShutdownSignal)`.
        let reg = registry_with_ping();
        let cursor = Cursor::new(String::new());
        let mut out: Vec<u8> = Vec::new();
        let signal = serve_stdio_with_io(&reg, std::io::BufReader::new(cursor), &mut out)
            .expect("clean shutdown");
        assert_eq!(signal, ShutdownSignal);
        assert!(out.is_empty(), "no frames written on empty input");
    }
}
