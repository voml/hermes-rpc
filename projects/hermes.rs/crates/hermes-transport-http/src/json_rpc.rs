//! JSON-RPC 2.0 wire types.
//!
//! 严格遵循 [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)：
//! - 所有请求必须携带 `"jsonrpc": "2.0"`
//! - 响应携带 `"jsonrpc": "2.0"` 并回填请求的 `id`
//! - 标准错误码：`-32700` / `-32600` / `-32601` / `-32602` / `-32603`
//! - 扩展错误码（业务层）保留 Hermes `RpcErrorCode` 作为 `data` 字段

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 协议版本，固定为 `"2.0"`
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC 2.0 标准错误码
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JsonRpcErrorCode {
    /// `-32700` — JSON 解析失败
    ParseError = -32700,
    /// `-32600` — 请求无效（协议版本缺失或不匹配）
    InvalidRequest = -32600,
    /// `-32601` — 方法不存在
    MethodNotFound = -32601,
    /// `-32602` — 参数无效
    InvalidParams = -32602,
    /// `-32603` — 内部错误
    InternalError = -32603,
}

/// JSON-RPC 2.0 请求体
///
/// `jsonrpc` 使用 `Option<String>` 以便 serde 在字段缺失时返回 `None` 而不是
/// 触发解析错误（这样 transport adapter 可以返回标准的 `-32600 Invalid Request`）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// 协议版本，必须为 `"2.0"`；缺失时为 `None`
    #[serde(default)]
    pub jsonrpc: Option<String>,
    /// 方法名，transport adapter 约定格式为 `"ServiceName.methodName"`
    pub method: String,
    /// 方法参数（任意 JSON 值）
    pub params: Value,
    /// 请求标识，响应时原样回填；notification 可为 `None`
    pub id: Option<Value>,
}

impl JsonRpcRequest {
    /// 判断是否为 notification（没有 `id` 字段）。
    ///
    /// JSON-RPC 2.0 spec §2.0：notification 请求**不应**产生任何响应
    /// —— HTTP 层应返回 204 No Content。
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// 校验协议版本是否存在且等于 `"2.0"`
    pub fn validate(&self) -> Result<(), JsonRpcError> {
        match self.jsonrpc.as_deref() {
            Some(JSONRPC_VERSION) => Ok(()),
            Some(other) => Err(JsonRpcError::new(
                JsonRpcErrorCode::InvalidRequest,
                format!("invalid jsonrpc version: expected \"{JSONRPC_VERSION}\", got \"{other}\""),
                None,
            )),
            None => Err(JsonRpcError::new(
                JsonRpcErrorCode::InvalidRequest,
                format!("missing jsonrpc version: expected \"{JSONRPC_VERSION}\""),
                None,
            )),
        }
    }
}

/// JSON-RPC 2.0 错误体
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// 错误码；`-32700` ~ `-32603` 为标准码，其余为扩展
    pub code: i32,
    /// 可读的错误描述
    pub message: String,
    /// 可选的附加数据，transport adapter 在此保留 Hermes `RpcErrorCode`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    /// 构造标准错误
    pub fn new(code: JsonRpcErrorCode, message: impl Into<String>, data: Option<Value>) -> Self {
        Self { code: code as i32, message: message.into(), data }
    }

    /// 构造扩展错误（保留 Hermes 错误码信息）
    pub fn ext(code: i32, message: impl Into<String>, data: Option<Value>) -> Self {
        Self { code, message: message.into(), data }
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

/// JSON-RPC 2.0 响应体
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// 协议版本，固定为 `"2.0"`
    pub jsonrpc: String,
    /// 成功时的返回值（与 `error` 互斥）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// 失败时的错误体（与 `result` 互斥）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// 请求 `id` 的原样回填
    pub id: Option<Value>,
}

impl JsonRpcResponse {
    /// 成功响应
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: JSONRPC_VERSION.to_string(), result: Some(result), error: None, id }
    }

    /// 错误响应
    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self { jsonrpc: JSONRPC_VERSION.to_string(), result: None, error: Some(error), id }
    }
}
