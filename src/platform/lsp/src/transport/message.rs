//! JSON-RPC messages.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// An error answer to a request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseError {
    /// JSON-RPC error code.
    pub code: i64,
    /// What went wrong.
    pub message: String,
    /// Anything else the server sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC: the method does not exist.
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
/// LSP: the request was cancelled.
pub(crate) const REQUEST_CANCELLED: i64 = -32800;
/// LSP: the document changed under the request; the answer is stale.
pub(crate) const CONTENT_MODIFIED: i64 = -32801;
/// LSP: the server cancelled the request.
pub(crate) const SERVER_CANCELLED: i64 = -32802;

/// A message from the server.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Incoming {
    /// The answer to one of our requests.
    Response {
        id: i64,
        result: Result<Value, ResponseError>,
    },
    /// The server asks us something; `id` must come back as it was.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// The server tells us something.
    Notification { method: String, params: Value },
}

impl Incoming {
    /// Parses a message body.
    pub(crate) fn parse(body: &[u8]) -> Result<Self, String> {
        let mut object: Map<String, Value> =
            serde_json::from_slice(body).map_err(|error| error.to_string())?;
        let method = match object.remove("method") {
            Some(Value::String(method)) => Some(method),
            Some(_) => return Err("method is not a string".to_owned()),
            None => None,
        };
        let params = object.remove("params").unwrap_or(Value::Null);
        let id = object.remove("id");
        match (method, id) {
            (Some(method), Some(id)) => Ok(Self::Request { id, method, params }),
            (Some(method), None) => Ok(Self::Notification { method, params }),
            (None, Some(id)) => {
                let id = id.as_i64().ok_or("response id is not ours")?;
                let result = if let Some(error) = object.remove("error") {
                    Err(serde_json::from_value(error).map_err(|error| error.to_string())?)
                } else {
                    // A missing result is `null` (some servers omit it).
                    Ok(object.remove("result").unwrap_or(Value::Null))
                };
                Ok(Self::Response { id, result })
            }
            (None, None) => Err("neither a request nor a response".to_owned()),
        }
    }
}

/// A request body.
pub(crate) fn request(id: i64, method: &str, params: &Value) -> Vec<u8> {
    body(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    }))
}

/// A notification body.
pub(crate) fn notification(method: &str, params: &Value) -> Vec<u8> {
    body(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    }))
}

/// The body of an answer to a server's request.
pub(crate) fn response(id: &Value, result: Result<Value, ResponseError>) -> Vec<u8> {
    match result {
        Ok(result) => body(&serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result })),
        Err(error) => body(&serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": error })),
    }
}

fn body(value: &Value) -> Vec<u8> {
    // Serializing a `Value` cannot fail.
    serde_json::to_vec(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(value: &Value) -> Incoming {
        Incoming::parse(&serde_json::to_vec(value).unwrap()).unwrap()
    }

    #[test]
    fn tells_messages_apart() {
        assert_eq!(
            parse(&json!({"jsonrpc": "2.0", "id": 3, "result": null})),
            Incoming::Response {
                id: 3,
                result: Ok(Value::Null)
            }
        );
        assert!(matches!(
            parse(&json!({"id": 4, "error": {"code": -32800, "message": "cancelled"}})),
            Incoming::Response {
                id: 4,
                result: Err(ResponseError {
                    code: REQUEST_CANCELLED,
                    ..
                })
            }
        ));
        assert!(matches!(
            parse(&json!({"id": "a", "method": "workspace/configuration", "params": {}})),
            Incoming::Request { .. }
        ));
        assert!(matches!(
            parse(&json!({"method": "$/progress"})),
            Incoming::Notification {
                params: Value::Null,
                ..
            }
        ));
        assert!(Incoming::parse(b"{}").is_err());
        assert!(Incoming::parse(b"[").is_err());
    }
}
