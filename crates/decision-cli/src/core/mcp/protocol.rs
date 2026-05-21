//! Minimal JSON-RPC 2.0 framing for the MCP server (FT-034).
//!
//! MCP transports messages as line-delimited JSON-RPC 2.0 envelopes
//! over stdio. This module defines just enough of that wire format to
//! implement the slice-1 scope: `initialize`, `notifications/initialized`
//! (handshake), `tools/list` (registry query), and `tools/call` (tool
//! invocation). Resources, prompts, sampling, and the remainder of the
//! MCP spec are out of scope per FT-034 §Out of scope.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 protocol version literal.
pub const JSONRPC_VERSION: &str = "2.0";

/// MCP protocol version this server speaks. Mirrors the wire constant
/// from the MCP spec (2024-11-05 revision). Clients that request a
/// newer version get the server's version back — per the MCP handshake
/// rules, mismatches are recoverable by the client.
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// A JSON-RPC id field. May be a string, integer, or null per the spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Id {
    /// Integer id (the common case for SDK-generated ids).
    Num(i64),
    /// String id.
    Text(String),
    /// Null id (used by notifications carried over the wire).
    Null,
}

/// Incoming JSON-RPC 2.0 request frame.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcRequest {
    /// Protocol version sentinel. Always `"2.0"` per spec; we accept
    /// other values and let the dispatcher decide. The field is read
    /// by serde during deserialisation and surfaces in tracing only,
    /// so static analysers flag it as "unused" — suppress that locally.
    #[allow(dead_code)]
    pub jsonrpc: String,
    /// Request id. Notifications omit this field; the deserialiser
    /// folds the missing field into `None`.
    #[serde(default)]
    pub id: Option<Id>,
    /// Method name (e.g. `tools/list`).
    pub method: String,
    /// Method parameters; absent for many methods.
    #[serde(default)]
    pub params: Option<Value>,
}

impl RpcRequest {
    /// True iff this frame is a notification (no `id`). Notifications
    /// must not be replied to per the JSON-RPC 2.0 spec.
    #[must_use]
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// Outgoing JSON-RPC 2.0 success response.
#[derive(Debug, Clone, Serialize)]
pub struct RpcResponse {
    /// Always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Echoed request id.
    pub id: Id,
    /// Method-specific structured result.
    pub result: Value,
}

impl RpcResponse {
    /// Construct a success response for `id` carrying `result`.
    pub fn ok(id: Id, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result,
        }
    }
}

/// Outgoing JSON-RPC 2.0 error response envelope.
#[derive(Debug, Clone, Serialize)]
pub struct RpcErrorFrame {
    /// Always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Echoed request id (or null when the originating id is unknown).
    pub id: Id,
    /// Structured error payload.
    pub error: RpcErrorBody,
}

/// JSON-RPC 2.0 error body. Codes follow the spec's reserved ranges:
/// -32600 invalid request, -32601 method not found, -32602 invalid
/// params, -32603 internal error, -32700 parse error. Application-
/// specific codes use the -32000..-32099 range.
#[derive(Debug, Clone, Serialize)]
pub struct RpcErrorBody {
    /// JSON-RPC error code.
    pub code: i32,
    /// Short human-readable message.
    pub message: String,
    /// Optional structured detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcErrorFrame {
    /// Build an error frame for `id`.
    pub fn new(id: Id, code: i32, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            error: RpcErrorBody {
                code,
                message: message.into(),
                data,
            },
        }
    }
}

/// JSON-RPC 2.0 method-not-found code.
pub const ERR_METHOD_NOT_FOUND: i32 = -32601;
/// JSON-RPC 2.0 invalid-params code.
pub const ERR_INVALID_PARAMS: i32 = -32602;
/// JSON-RPC 2.0 internal-error code.
pub const ERR_INTERNAL: i32 = -32603;
/// JSON-RPC 2.0 parse-error code (malformed frame).
pub const ERR_PARSE: i32 = -32700;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_request_with_int_id() {
        let raw = r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#;
        let req: RpcRequest = serde_json::from_str(raw).expect("parse");
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.id, Some(Id::Num(7)));
        assert!(!req.is_notification());
    }

    #[test]
    fn parses_notification_without_id() {
        let raw = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: RpcRequest = serde_json::from_str(raw).expect("parse");
        assert!(req.is_notification());
    }

    #[test]
    fn serialises_ok_response() {
        let resp = RpcResponse::ok(Id::Num(7), json!({"tools": []}));
        let raw = serde_json::to_string(&resp).expect("serialise");
        assert!(raw.contains("\"jsonrpc\":\"2.0\""));
        assert!(raw.contains("\"id\":7"));
        assert!(raw.contains("\"result\":{\"tools\":[]}"));
    }

    #[test]
    fn serialises_error_frame() {
        let resp = RpcErrorFrame::new(Id::Num(7), ERR_METHOD_NOT_FOUND, "no such method", None);
        let raw = serde_json::to_string(&resp).expect("serialise");
        assert!(raw.contains("\"code\":-32601"));
        assert!(raw.contains("\"message\":\"no such method\""));
    }
}
