//! HTTP/JSON-RPC 2.0 transport adapter。
//!
//! 本层只负责协议适配：JSON-RPC 请求体 → Hermes dispatcher → JSON-RPC 响应体。
//! 不修改 `hermes-runtime` 核心逻辑，dispatcher 保持纯粹的 `service.method` 路由。

use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use hermes_runtime::{DispatchReply, HermesDispatcher, RpcErrorCode, RpcError};
use serde_json::Value;

use crate::json_rpc::{
    JsonRpcError, JsonRpcErrorCode, JsonRpcRequest, JsonRpcResponse,
};

/// HTTP transport 配置
#[derive(Clone, Debug)]
pub struct HttpTransportConfig {
    /// 绑定地址，测试常用 `"127.0.0.1:0"`
    pub bind_addr: String,
}

impl Default for HttpTransportConfig {
    fn default() -> Self {
        Self { bind_addr: "127.0.0.1:0".to_string() }
    }
}

/// HTTP/JSON-RPC 2.0 transport，持有 Hermes dispatcher 的只读引用
#[derive(Clone)]
pub struct HttpTransport {
    dispatcher: Arc<HermesDispatcher>,
}

impl HttpTransport {
    /// 构造 adapter。外部负责提前向 dispatcher 注册 handler 与加载合同。
    pub fn new(dispatcher: Arc<HermesDispatcher>) -> Self {
        Self { dispatcher }
    }

    /// 生成 axum Router，挂在 `/rpc`
    pub fn router(&self) -> Router {
        Router::new().route("/rpc", post(rpc_handler)).with_state(self.clone())
    }

    /// 入口：原始 JSON 文本 → JSON-RPC 请求 → dispatcher → JSON-RPC 响应。
    ///
    /// 这是给调用方直接用的（不走 HTTP server），便于单元测试。
    pub fn handle_raw(&self, body: &str) -> JsonRpcResponse {
        // 1. JSON-RPC 解析（-32700）
        let request = match serde_json::from_str::<JsonRpcRequest>(body) {
            Ok(req) => req,
            Err(err) => {
                return JsonRpcResponse::error(
                    None,
                    JsonRpcError::new(JsonRpcErrorCode::ParseError, err.to_string(), None),
                );
            }
        };

        self.handle(request)
    }

    /// 处理单个 JSON-RPC 请求：拆分 `service.method` → dispatch → 映射响应
    pub fn handle(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        // 2. 协议版本校验（-32600）
        if let Err(err) = request.validate() {
            return JsonRpcResponse::error(id, err);
        }

        // 3. 拆分 method 字段 → service + method
        let Some((service, method)) = split_method(&request.method) else {
            return JsonRpcResponse::error(
                id,
                JsonRpcError::new(
                    JsonRpcErrorCode::InvalidRequest,
                    format!("invalid method format: expected \"Service.method\", got \"{}\"", request.method),
                    None,
                ),
            );
        };

        // 4. dispatch
        match self.dispatcher.dispatch(service, method, &request.params) {
            Ok(DispatchReply::Unary(value)) => JsonRpcResponse::success(id, value),
            Ok(DispatchReply::ServerStream(items)) => {
                // JSON-RPC 2.0 单请求返回数组，骨架阶段直接把 stream 当数组返回
                JsonRpcResponse::success(id, Value::Array(items))
            }
            Err(err) => JsonRpcResponse::error(id, map_rpc_error_to_jsonrpc(err)),
        }
    }
}

/// 把 `"Service.method"` 拆成 `("Service", "method")`
/// 按第一个 `.` 拆分，允许多段 `ns.Service.method` → (`ns.Service`, `method`)
fn split_method(method: &str) -> Option<(&str, &str)> {
    let parts = method.splitn(2, '.').collect::<Vec<_>>();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0], parts[1]))
    } else {
        None
    }
}

/// Hermes `RpcError` → JSON-RPC 2.0 错误
fn map_rpc_error_to_jsonrpc(err: RpcError) -> JsonRpcError {
    match err.code() {
        RpcErrorCode::UnknownMethod | RpcErrorCode::UnknownService => {
            JsonRpcError::new(JsonRpcErrorCode::MethodNotFound, err.message().to_string(), None)
        }
        RpcErrorCode::InvalidRequest => {
            JsonRpcError::new(JsonRpcErrorCode::InvalidParams, err.message().to_string(), None)
        }
        other => {
            // 保留 Hermes 错误码在 data 里，便于宿主做更细粒度处理
            let data = serde_json::json!({
                "hermes_code": other.as_str(),
                "diagnostic_code": err.diagnostic_code(),
            });
            JsonRpcError::new(JsonRpcErrorCode::InternalError, err.message().to_string(), Some(data))
        }
    }
}

// ---------------------------------------------------------------------------
// axum handler
// ---------------------------------------------------------------------------

async fn rpc_handler(
    State(transport): State<HttpTransport>,
    body: String,
) -> (StatusCode, Json<JsonRpcResponse>) {
    let response = transport.handle_raw(&body);
    let status = match &response.error {
        None => StatusCode::OK,
        // -32700 / -32600 在 HTTP 层仍然返回 200（JSON-RPC 规范）
        _ => StatusCode::OK,
    };
    (status, Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_method_handles_simple_and_nested_namespaces() {
        assert_eq!(split_method("IdentityService.create"), Some(("IdentityService", "create")));
        assert_eq!(split_method("ns.IdentityService.create"), Some(("ns", "IdentityService.create")));
        assert_eq!(split_method("create"), None);
        assert_eq!(split_method(".create"), None);
        assert_eq!(split_method("Service."), None);
        assert_eq!(split_method(""), None);
    }

    #[test]
    fn map_rpc_error_maps_known_codes_to_jsonrpc_standard() {
        let not_found = RpcError::new(RpcErrorCode::UnknownMethod, "no such method");
        let mapped = map_rpc_error_to_jsonrpc(not_found);
        assert_eq!(mapped.code, JsonRpcErrorCode::MethodNotFound as i32);
        assert!(mapped.data.is_none());

        let invalid_req = RpcError::new(RpcErrorCode::InvalidRequest, "missing param");
        let mapped = map_rpc_error_to_jsonrpc(invalid_req);
        assert_eq!(mapped.code, JsonRpcErrorCode::InvalidParams as i32);

        let handler_fail = RpcError::new(RpcErrorCode::HandlerFailed, "boom");
        let mapped = map_rpc_error_to_jsonrpc(handler_fail);
        assert_eq!(mapped.code, JsonRpcErrorCode::InternalError as i32);
        assert!(mapped.data.is_some());
    }

    #[test]
    fn json_rpc_response_round_trips_through_serde() {
        let success = JsonRpcResponse::success(Some(Value::Number(42.into())), Value::String("ok".into()));
        let json = serde_json::to_string(&success).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        let back: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, success);
    }
}
