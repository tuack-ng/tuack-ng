use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JSONRPC: &str = "2.0";

// 标准错误码
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
// 内部错误码
pub const INTERNAL_ERROR: i64 = -32000;
// 业务错误码
pub const SESSION_NOT_FOUND: i64 = -32001;
pub const INVALID_PROJECT: i64 = -32002;
pub const COMPILE_ERROR: i64 = -32003;
pub const RUN_ERROR: i64 = -32004;
pub const INVALID_CONFIG: i64 = -32005;
pub const RUN_NOT_FOUND: i64 = -32006;
pub const REVISION_CONFLICT: i64 = -32007;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Id {
    Number(u64),
    String(String),
    Null,
}

#[derive(Debug, Deserialize)]
pub struct Request {
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: String,
    pub id: Id,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Notification {
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    Request(Request),
    Notification(Notification),
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        RpcError {
            code,
            message: message.into(),
            data: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: Id, result: Value) -> Self {
        Response {
            jsonrpc: JSONRPC,
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Id, error: RpcError) -> Self {
        Response {
            jsonrpc: JSONRPC,
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: Value,
}

impl Event {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Event {
            jsonrpc: JSONRPC,
            method: method.into(),
            params,
        }
    }
}
