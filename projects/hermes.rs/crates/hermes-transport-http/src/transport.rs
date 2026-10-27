//! HTTP/JSON-RPC 2.0 transport adapter。
//!
//! 本层只负责协议适配：JSON-RPC 请求体 → Hermes dispatcher → JSON-RPC 响应体。
//! 不修改 `hermes-runtime` 核心逻辑，dispatcher 保持纯粹的 `service.method` 路由。
//!
//! 覆盖 JSON-RPC 2.0 spec 的全部语义：
//! - 单请求 / 单响应
//! - **notification**（无 `id` 字段）→ 不产生任何响应，HTTP 返回 204 No Content
//! - **batch**（请求体是 JSON 数组）→ 每个请求单独 dispatch，收集非 notification 响应
//!   - 数组为空 `[]` → -32600 Invalid Request
//!   - 数组全为 notification → 204 No Content
//!   - batch 中每个无效元素各自返回对应错误码
//! - 根请求体不是合法 JSON → -32700 Parse Error
//! - 根请求体是合法 JSON 但不是 object 或 array → -32600 Invalid Request

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use hermes_runtime::{DispatchReply, HermesDispatcher, RpcError, RpcErrorCode};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::task::JoinHandle;

use crate::json_rpc::{JsonRpcError, JsonRpcErrorCode, JsonRpcRequest, JsonRpcResponse};

/// `handle_raw` 的三种可能结果
///
/// JSON-RPC 2.0 允许的三种 HTTP 响应形状：
/// - 单请求有响应 → 单个 `JsonRpcResponse`
/// - batch 有响应 → `Vec<JsonRpcResponse>`（注意：**排除 notification**）
/// - notification 或 batch 全 notification → 无 body，HTTP 204
#[derive(Clone, Debug)]
pub enum HandleOutcome {
    /// 单请求产生了响应（包括错误响应）
    Single(JsonRpcResponse),
    /// batch 请求产生了至少一个响应
    Batch(Vec<JsonRpcResponse>),
    /// notification 或 batch 全部是 notification —— 不返回任何 body
    NoContent,
}

/// 把 `HandleOutcome` 映射成 HTTP 响应：
/// - `Single` → 200 + JSON body
/// - `Batch`  → 200 + JSON array body
/// - `NoContent` → 204 No Content
impl IntoResponse for HandleOutcome {
    fn into_response(self) -> axum::response::Response {
        match self {
            HandleOutcome::Single(resp) => (StatusCode::OK, Json(resp)).into_response(),
            HandleOutcome::Batch(vec) => (StatusCode::OK, Json(vec)).into_response(),
            HandleOutcome::NoContent => StatusCode::NO_CONTENT.into_response(),
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP transport config
// ---------------------------------------------------------------------------

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

/// HTTP server 启动错误。
#[derive(Debug)]
pub enum TransportError {
    /// `bind_addr` 无法解析成 `SocketAddr`
    InvalidAddress(String),
    /// TCP 绑定或监听失败
    Io(io::Error),
    /// axum `serve` 返回错误
    Serve(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAddress(addr) => write!(f, "invalid bind address: `{addr}`"),
            Self::Io(err) => write!(f, "transport I/O error: {err}"),
            Self::Serve(err) => write!(f, "axum serve error: {err}"),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<io::Error> for TransportError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
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

    /// 启动 HTTP server，阻塞直到 shutdown signal 或错误。
    ///
    /// 绑定 `config.bind_addr`（`"127.0.0.1:0"` 让 OS 分配随机端口），然后用 axum
    /// 监听。收到 **Ctrl+C**（所有平台）或 **SIGTERM**（Unix）时触发 graceful
    /// shutdown：拒绝新连接，等待 in-flight 请求完成（由 axum 默认超时控制），
    /// 最后返回 `Ok(())`。
    pub async fn listen(self, config: HttpTransportConfig) -> Result<(), TransportError> {
        let addr: SocketAddr = config.bind_addr.parse().map_err(|_| TransportError::InvalidAddress(config.bind_addr))?;
        let listener = TcpListener::bind(addr).await?;

        let shutdown = async {
            // ctrl_c() 跨平台：Windows 捕获 Ctrl+C；Unix 额外监听 SIGINT + SIGTERM。
            let _ = signal::ctrl_c().await;
        };

        axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|err| TransportError::Serve(Box::new(err)))
    }

    /// 非阻塞版本：在当前 tokio runtime 上起一个后台任务运行 server。
    ///
    /// 返回的 `JoinHandle<()>` 可以用来 `await` 或 `.abort()`。server 正常退出（收到
    /// shutdown signal）时 handle resolve；遇到启动或 serve 阶段的 error 时，error
    /// 会被 `tracing` 输出（本 crate 不强制引入 tracing，只通过 `log` 打印到 stderr）。
    pub fn serve_background(self, config: HttpTransportConfig) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(err) = self.listen(config).await {
                eprintln!("[hermes-transport-http] server error: {err}");
            }
        })
    }

    /// 入口：原始 JSON 文本 → 单请求 / batch / notification 判定 → dispatch → `HandleOutcome`。
    ///
    /// 这是给调用方直接用的（不走 HTTP server），便于单元测试覆盖 batch / notification 语义。
    pub fn handle_raw(&self, body: &str) -> HandleOutcome {
        // 1. 解析顶层 JSON —— 失败 → -32700 Parse Error（单请求错误，不是 batch-level）
        let top_value = match serde_json::from_str::<Value>(body) {
            Ok(v) => v,
            Err(err) => {
                return HandleOutcome::Single(JsonRpcResponse::error(
                    None,
                    JsonRpcError::new(JsonRpcErrorCode::ParseError, err.to_string(), None),
                ));
            }
        };

        match top_value {
            // --- batch 路径 ---------------------------------------------------
            Value::Array(batch) => self.handle_batch(batch),

            // --- 单请求路径 ---------------------------------------------------
            Value::Object(_) => self.handle_single_value(&top_value),

            // 合法 JSON 但根不是 object 或 array → -32600 Invalid Request
            // 这覆盖 `"hello"` / `42` / `null` 等顶层值 —— 不是有效的 JSON-RPC 请求
            _ => HandleOutcome::Single(JsonRpcResponse::error(
                None,
                JsonRpcError::new(
                    JsonRpcErrorCode::InvalidRequest,
                    "request must be a JSON object or a JSON array".to_string(),
                    None,
                ),
            )),
        }
    }

    /// 处理单个 JSON-RPC 请求对象（已经是 `Value::Object`）
    fn handle_single_value(&self, value: &Value) -> HandleOutcome {
        // 2. 解析成 JsonRpcRequest
        let request: JsonRpcRequest = match serde_json::from_value(value.clone()) {
            Ok(req) => req,
            // 能到达这里说明 JSON 值合法，但缺少必需字段（`method`）或形状不对
            // → -32600 Invalid Request
            Err(err) => {
                return HandleOutcome::Single(JsonRpcResponse::error(
                    None,
                    JsonRpcError::new(JsonRpcErrorCode::InvalidRequest, format!("invalid JSON-RPC request: {err}"), None),
                ));
            }
        };

        // 3. notification → 无论 validate/dispatch 成功失败都不返回响应（spec §2.0）
        if request.is_notification() {
            // 仍然 dispatch 让 handler 有机会执行（side effects），但丢弃结果
            let _ = self.dispatch_internal(&request);
            return HandleOutcome::NoContent;
        }

        // 4. 普通请求 → 正常 dispatch 并返回响应
        HandleOutcome::Single(self.dispatch_internal(&request))
    }

    /// 处理 batch 请求（JSON 数组）
    ///
    /// JSON-RPC 2.0 spec §2.1：
    /// - 空数组 `[]` → -32600 Invalid Request（单请求错误，不是 batch-level）
    /// - 每个元素独立 dispatch
    /// - **notification 不进入响应数组**
    /// - 全 notification → 204 No Content
    /// - 至少一个有响应 → 所有响应的 JSON 数组
    fn handle_batch(&self, batch: Vec<Value>) -> HandleOutcome {
        // 空 batch → -32600（按 spec 当作"无效请求"处理，不是 -32700）
        if batch.is_empty() {
            return HandleOutcome::Single(JsonRpcResponse::error(
                None,
                JsonRpcError::new(JsonRpcErrorCode::InvalidRequest, "empty batch request: []".to_string(), None),
            ));
        }

        let mut responses: Vec<JsonRpcResponse> = Vec::new();

        for elem in batch {
            // 每个元素尝试 parse 成 JsonRpcRequest
            let request: JsonRpcRequest = match serde_json::from_value(elem) {
                Ok(req) => req,
                // batch 中某个元素不是合法的 JSON-RPC request object → -32600
                Err(err) => {
                    responses.push(JsonRpcResponse::error(
                        None,
                        JsonRpcError::new(JsonRpcErrorCode::InvalidRequest, format!("invalid batch element: {err}"), None),
                    ));
                    continue;
                }
            };

            // notification 跳过（不 dispatch 也不收集——它没有响应）
            if request.is_notification() {
                let _ = self.dispatch_internal(&request);
                continue;
            }

            // 普通请求：dispatch 并收集响应
            responses.push(self.dispatch_internal(&request));
        }

        if responses.is_empty() {
            // 全是 notification
            HandleOutcome::NoContent
        } else {
            HandleOutcome::Batch(responses)
        }
    }

    /// 内部共用：对已经解析好的 `JsonRpcRequest` 做 validate + split_method + dispatch
    ///
    /// 返回单个 `JsonRpcResponse`（可能是成功或错误）。调用方负责决定这个响应是否应该被
    /// 收集进 batch 数组或直接返回给 HTTP 层。
    fn dispatch_internal(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone();

        // 协议版本校验（-32600）
        if let Err(err) = request.validate() {
            return JsonRpcResponse::error(id, err);
        }

        // method 拆分（-32600）
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

        // dispatch
        match self.dispatcher.dispatch(service, method, &request.params) {
            Ok(DispatchReply::Unary(value)) => JsonRpcResponse::success(id, value),
            Ok(DispatchReply::ServerStream(items)) => {
                // JSON-RPC 2.0 单请求返回数组，骨架阶段直接把 stream 当数组返回
                JsonRpcResponse::success(id, Value::Array(items))
            }
            Err(err) => JsonRpcResponse::error(id, map_rpc_error_to_jsonrpc(err)),
        }
    }

    /// 处理单个已解析的 JSON-RPC 请求 —— 公开 API（给不想走 JSON 解析的调用方）
    ///
    /// 注意：此方法始终返回一个 `JsonRpcResponse`（notification 也会被正常 dispatch
    /// 并返回结果）。如果需要完整的 batch/notification 语义，请用 `handle_raw`。
    pub fn handle(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        self.dispatch_internal(&request)
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 把 `"Service.method"` 拆成 `("Service", "method")`
/// 按第一个 `.` 拆分，允许多段 `ns.Service.method` → (`ns.Service`, `method`)
fn split_method(method: &str) -> Option<(&str, &str)> {
    let parts = method.splitn(2, '.').collect::<Vec<_>>();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() { Some((parts[0], parts[1])) } else { None }
}

/// Hermes `RpcError` → JSON-RPC 2.0 错误
fn map_rpc_error_to_jsonrpc(err: RpcError) -> JsonRpcError {
    match err.code() {
        RpcErrorCode::UnknownMethod | RpcErrorCode::UnknownService => {
            JsonRpcError::new(JsonRpcErrorCode::MethodNotFound, err.message().to_string(), None)
        }
        RpcErrorCode::InvalidRequest => JsonRpcError::new(JsonRpcErrorCode::InvalidParams, err.message().to_string(), None),
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

async fn rpc_handler(State(transport): State<HttpTransport>, body: String) -> impl IntoResponse {
    transport.handle_raw(&body)
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

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

    // --- notification / batch unit tests ------------------------------------

    #[test]
    fn value_non_object_non_array_is_invalid_request() {
        // 顶层是 string / number / null —— 合法 JSON 但不是合法 JSON-RPC 请求
        let dispatcher = Arc::new(HermesDispatcher::new());
        let transport = HttpTransport::new(dispatcher);

        match transport.handle_raw(r#""hello""#) {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error, got {other:?}"),
        }

        match transport.handle_raw("42") {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error, got {other:?}"),
        }

        match transport.handle_raw("null") {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error, got {other:?}"),
        }
    }

    #[test]
    fn empty_batch_is_invalid_request() {
        let dispatcher = Arc::new(HermesDispatcher::new());
        let transport = HttpTransport::new(dispatcher);

        match transport.handle_raw("[]") {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error for empty batch, got {other:?}"),
        }
    }
}
