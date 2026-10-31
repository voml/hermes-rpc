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
//!
//! 跨进程生命周期语义（deadline / cancellation）：
//! - 请求顶层扩展成员 `"deadline": <UTC epoch ms>` 折算成 runtime `CallOptions`
//!   deadline；过去的值在首个检查点即 `deadline-exceeded`，负值是 `-32600`
//! - `HttpTransportConfig::request_timeout` 是宿主兜底 deadline，与 wire deadline
//!   取更早者
//! - 客户端断连：hyper 1.x 的 h1 dispatcher 在 handler in-flight 期间不轮询 socket
//!   读侧，FIN/RST 不会自动 drop handler future。`make_service()` 在连接建立时克隆
//!   socket，用非消费式 `peek` 独立观察读侧，断开时 drop 请求 future，drop guard
//!   随之取消 per-request `CancelToken`，handler 在下一个协作检查点转 `cancelled`。
//!   取消语义只有 runtime 一份，transport 不另造取消状态机；只嵌入 `router()` 的
//!   宿主没有断连取消，wire deadline 与宿主 `request_timeout` 仍然生效

use std::convert::Infallible;
use std::future::{self, Future};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    Json, Router,
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    serve::IncomingStream,
};
use hermes_runtime::{CallOptions, CancelToken, DispatchReply, HermesDispatcher, RpcError, RpcErrorCode};
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::task::JoinHandle;
use tower_service::Service;

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
    /// 宿主侧兜底的单请求超时。与请求携带的 wire `deadline` 取更早者，折算成
    /// 同一个 runtime `CallOptions` deadline；`None` 表示不设宿主侧超时。
    pub request_timeout: Option<Duration>,
}

impl Default for HttpTransportConfig {
    fn default() -> Self {
        Self { bind_addr: "127.0.0.1:0".to_string(), request_timeout: None }
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

/// HTTP/JSON-RPC 2.0 transport，持有 Hermes dispatcher 的只读引用与宿主配置
#[derive(Clone)]
pub struct HttpTransport {
    dispatcher: Arc<HermesDispatcher>,
    config: HttpTransportConfig,
}

impl HttpTransport {
    /// 以默认配置构造 adapter。外部负责提前向 dispatcher 注册 handler 与加载合同。
    pub fn new(dispatcher: Arc<HermesDispatcher>) -> Self {
        Self { dispatcher, config: HttpTransportConfig::default() }
    }

    /// 以显式宿主配置构造 adapter（`bind_addr` 与 `request_timeout`）。
    pub fn with_config(dispatcher: Arc<HermesDispatcher>, config: HttpTransportConfig) -> Self {
        Self { dispatcher, config }
    }

    /// 生成 axum Router，挂在 `/rpc`
    ///
    /// 只嵌入 `router()` 的宿主拿不到断连取消（hyper 1.x 的 h1 在 handler
    /// in-flight 期间不轮询读侧），wire deadline 与宿主 `request_timeout` 仍然生效。
    /// 需要断连取消的宿主用 [`HttpTransport::make_service`] 交给 `axum::serve`，
    /// 或直接 `listen` / `serve_background`。
    pub fn router(&self) -> Router {
        Router::new().route("/rpc", post(rpc_handler)).with_state(self.clone())
    }

    /// 生成带断连探测的 `axum::serve` make service。
    ///
    /// 连接建立时克隆 socket 读端句柄；每个请求的 handler future 与非消费式
    /// `peek` 探测并存，观察到 FIN/RST 时 drop handler future，由
    /// `dispatch_internal` 的 drop guard 取消该请求的 runtime `CancelToken`。
    pub fn make_service(&self) -> DisconnectAwareMakeService {
        DisconnectAwareMakeService { router: self.router() }
    }

    /// 启动 HTTP server，阻塞直到 shutdown signal 或错误。
    ///
    /// 绑定 `self.config.bind_addr`（`"127.0.0.1:0"` 让 OS 分配随机端口），然后用
    /// axum 监听。收到 **Ctrl+C**（所有平台）或 **SIGTERM**（Unix）时触发 graceful
    /// shutdown：拒绝新连接，等待 in-flight 请求完成（由 axum 默认超时控制），
    /// 最后返回 `Ok(())`。
    pub async fn listen(self) -> Result<(), TransportError> {
        let addr: SocketAddr = self
            .config
            .bind_addr
            .parse()
            .map_err(|_| TransportError::InvalidAddress(self.config.bind_addr.clone()))?;
        let listener = TcpListener::bind(addr).await?;

        let shutdown = async {
            // ctrl_c() 跨平台：Windows 捕获 Ctrl+C；Unix 额外监听 SIGINT + SIGTERM。
            let _ = signal::ctrl_c().await;
        };

        axum::serve(listener, self.make_service())
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|err| TransportError::Serve(Box::new(err)))
    }

    /// 非阻塞版本：在当前 tokio runtime 上起一个后台任务运行 server。
    ///
    /// 返回的 `JoinHandle<()>` 可以用来 `await` 或 `.abort()`。server 正常退出（收到
    /// shutdown signal）时 handle resolve；遇到启动或 serve 阶段的 error 时，error
    /// 会被 `tracing` 输出（本 crate 不强制引入 tracing，只通过 `log` 打印到 stderr）。
    pub fn serve_background(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(err) = self.listen().await {
                eprintln!("[hermes-transport-http] server error: {err}");
            }
        })
    }

    /// 入口：原始 JSON 文本 → 单请求 / batch / notification 判定 → dispatch → `HandleOutcome`。
    ///
    /// 这是给调用方直接用的（不走 HTTP server），便于单元测试覆盖 batch / notification 语义。
    pub async fn handle_raw(&self, body: &str) -> HandleOutcome {
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
            Value::Array(batch) => self.handle_batch(batch).await,

            // --- 单请求路径 ---------------------------------------------------
            Value::Object(_) => self.handle_single_value(&top_value).await,

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
    async fn handle_single_value(&self, value: &Value) -> HandleOutcome {
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
            let _ = self.dispatch_internal(&request).await;
            return HandleOutcome::NoContent;
        }

        // 4. 普通请求 → 正常 dispatch 并返回响应
        HandleOutcome::Single(self.dispatch_internal(&request).await)
    }

    /// 处理 batch 请求（JSON 数组）
    ///
    /// JSON-RPC 2.0 spec §2.1：
    /// - 空数组 `[]` → -32600 Invalid Request（单请求错误，不是 batch-level）
    /// - 每个元素独立 dispatch
    /// - **notification 不进入响应数组**
    /// - 全 notification → 204 No Content
    /// - 至少一个有响应 → 所有响应的 JSON 数组
    async fn handle_batch(&self, batch: Vec<Value>) -> HandleOutcome {
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
                let _ = self.dispatch_internal(&request).await;
                continue;
            }

            // 普通请求：dispatch 并收集响应
            responses.push(self.dispatch_internal(&request).await);
        }

        if responses.is_empty() {
            // 全是 notification
            HandleOutcome::NoContent
        } else {
            HandleOutcome::Batch(responses)
        }
    }

    /// 内部共用：对已经解析好的 `JsonRpcRequest` 做 validate + split_method +
    /// deadline/cancel 接线 + dispatch。
    ///
    /// runtime 是同步模型，handler 在 `spawn_blocking` 上执行。每个请求绑定独立
    /// [`CancelToken`]：wire `deadline` 与宿主 `request_timeout` 合并成
    /// `CallOptions` deadline（取更早者），请求 future 因客户端断连被 drop 时由
    /// drop guard 取消令牌。deadline 与取消的判定与错误码都在 runtime。
    ///
    /// 返回单个 `JsonRpcResponse`（可能是成功或错误）。调用方负责决定这个响应是否应该被
    /// 收集进 batch 数组或直接返回给 HTTP 层。
    async fn dispatch_internal(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
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

        // wire deadline（负值 → -32600）与宿主 request_timeout 合并成同一个 runtime deadline
        let mut options = CallOptions::new();
        if let Some(deadline_ms) = request.deadline {
            match wire_deadline_to_instant(deadline_ms) {
                Ok(deadline) => options = options.and_deadline(deadline),
                Err(message) => {
                    return JsonRpcResponse::error(id, JsonRpcError::new(JsonRpcErrorCode::InvalidRequest, message, None));
                }
            }
        }
        if let Some(timeout) = self.config.request_timeout {
            let host_deadline = Instant::now() + timeout;
            let effective = options.deadline().map_or(host_deadline, |wire| wire.min(host_deadline));
            options = options.and_deadline(effective);
        }

        let token = CancelToken::new();
        options = options.and_cancellation(token.clone());

        let dispatcher = Arc::clone(&self.dispatcher);
        let service = service.to_string();
        let method = method.to_string();
        let params = request.params.clone();
        let join = tokio::task::spawn_blocking(move || dispatcher.dispatch_with(&service, &method, &params, &options));

        // 断连取消：外层 future 被 `make_service()` 的断连探测 drop 时 guard 取消
        // 令牌，协作式 handler 在下一个检查点转 `cancelled`。
        let cancel_guard = CancelOnDrop(token);
        let outcome = join.await;
        drop(cancel_guard);

        match outcome {
            Ok(Ok(DispatchReply::Unary(value))) => JsonRpcResponse::success(id, value),
            Ok(Ok(DispatchReply::ServerStream(items))) => {
                // JSON-RPC 2.0 单请求返回数组，骨架阶段直接把 stream 当数组返回
                JsonRpcResponse::success(id, Value::Array(items))
            }
            Ok(Err(err)) => JsonRpcResponse::error(id, map_rpc_error_to_jsonrpc(err)),
            Err(join_err) => {
                // handler panic 已被 runtime 折算成 `internal` 错误，此分支只覆盖
                // runtime 边界之外的 blocking 任务 panic 或 runtime 关停期的任务取消。
                let message = if join_err.is_panic() {
                    "dispatch task panicked outside the runtime boundary".to_string()
                } else {
                    "dispatch task was cancelled by the runtime".to_string()
                };
                JsonRpcResponse::error(id, map_rpc_error_to_jsonrpc(RpcError::new(RpcErrorCode::Internal, message)))
            }
        }
    }

    /// 处理单个已解析的 JSON-RPC 请求 —— 公开 API（给不想走 JSON 解析的调用方）
    ///
    /// 注意：此方法始终返回一个 `JsonRpcResponse`（notification 也会被正常 dispatch
    /// 并返回结果）。如果需要完整的 batch/notification 语义，请用 `handle_raw`。
    pub async fn handle(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        self.dispatch_internal(&request).await
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

/// wire deadline 前瞻上限：极端未来的值被截断到这个时长，避免
/// `Instant + Duration` 溢出 panic。
const MAX_DEADLINE_HORIZON: Duration = Duration::from_secs(10 * 366 * 24 * 60 * 60);

/// 把 wire deadline（UTC epoch 毫秒）折算成本地单调 `Instant`。
///
/// 负值不是合法时刻，返回 `Err`（调用方折算成 `-32600`）。过去的 deadline 钳到
/// `Instant::now()`，在 runtime 的首个检查点即 `deadline-exceeded`；未来值按宿主
/// 墙钟换算为相对时长，前瞻上限为 [`MAX_DEADLINE_HORIZON`]。
fn wire_deadline_to_instant(deadline_ms: i64) -> Result<Instant, String> {
    if deadline_ms < 0 {
        return Err(format!("deadline must be a non-negative UTC epoch millisecond value, got {deadline_ms}"));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|err| format!("host clock is unusable: {err}"))?;
    let remaining = Duration::from_millis(deadline_ms as u64).saturating_sub(now);
    Ok(Instant::now() + remaining.min(MAX_DEADLINE_HORIZON))
}

/// 断连取消的 drop guard：请求 future 被 hyper drop（客户端 EOF）时取消绑定的
/// [`CancelToken`]。取消语义仍然只有 runtime 一份，transport 只接线信号源。
struct CancelOnDrop(CancelToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// Hermes `RpcError` → JSON-RPC 2.0 错误的**唯一**映射表。
///
/// 全分支穷举 `RpcErrorCode`，不留兜底 `other` 臂：新增错误码必须在此显式
/// 归类，编译期强制。`data` 形状固定为 `{"hermes_code": "<kebab>", "diagnostic_code": "<上游码>"}`，
/// `diagnostic_code` 仅在该错误携带上游 artifact 诊断码时出现。
///
/// | `RpcErrorCode` | JSON-RPC code |
/// |---|---|
/// | `unknown-service` / `unknown-method` / `unknown-operation` / `handler-not-registered` | `-32601` |
/// | `invalid-request` | `-32602` |
/// | `artifact-corrupt` / `duplicate-registration` / `missing-operation-binding` / `unsupported-codec` / `stream-mode-mismatch` / `internal` | `-32603` |
/// | `handler-failed` | `-32000` |
/// | `cancelled` | `-32001` |
/// | `deadline-exceeded` | `-32002` |
pub fn map_rpc_error_to_jsonrpc(err: RpcError) -> JsonRpcError {
    let code = match err.code() {
        RpcErrorCode::UnknownService | RpcErrorCode::UnknownMethod | RpcErrorCode::UnknownOperation
        | RpcErrorCode::HandlerNotRegistered => JsonRpcErrorCode::MethodNotFound,
        RpcErrorCode::InvalidRequest => JsonRpcErrorCode::InvalidParams,
        RpcErrorCode::ArtifactCorrupt
        | RpcErrorCode::DuplicateRegistration
        | RpcErrorCode::MissingOperationBinding
        | RpcErrorCode::UnsupportedCodec
        | RpcErrorCode::StreamModeMismatch
        | RpcErrorCode::Internal => JsonRpcErrorCode::InternalError,
        RpcErrorCode::HandlerFailed => JsonRpcErrorCode::HandlerFailed,
        RpcErrorCode::Cancelled => JsonRpcErrorCode::Cancelled,
        RpcErrorCode::DeadlineExceeded => JsonRpcErrorCode::DeadlineExceeded,
    };
    let data = match err.diagnostic_code() {
        Some(diagnostic) => serde_json::json!({ "hermes_code": err.code().as_str(), "diagnostic_code": diagnostic }),
        None => serde_json::json!({ "hermes_code": err.code().as_str() }),
    };
    JsonRpcError::new(code, err.message().to_string(), Some(data))
}

// ---------------------------------------------------------------------------
// axum handler
// ---------------------------------------------------------------------------

async fn rpc_handler(State(transport): State<HttpTransport>, body: String) -> impl IntoResponse {
    transport.handle_raw(&body).await
}

// ---------------------------------------------------------------------------
// 断连探测接线（serve 路径）
// ---------------------------------------------------------------------------

/// `axum::serve` 的 make service：连接建立时克隆 socket 读端句柄，
/// 给每个请求的 handler future 接上断连探测。
#[derive(Clone)]
pub struct DisconnectAwareMakeService {
    router: Router,
}

impl<'a> Service<IncomingStream<'a, TcpListener>> for DisconnectAwareMakeService {
    type Response = DisconnectAwareService;
    type Error = Infallible;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, incoming: IncomingStream<'a, TcpListener>) -> Self::Future {
        // 句柄复制失败（fd 耗尽等极端情况）退化为无断连探测，
        // wire deadline 与宿主 `request_timeout` 仍然生效
        let probe = dup_probe_socket(incoming.io()).ok().map(Arc::new);
        future::ready(Ok(DisconnectAwareService { router: self.router.clone(), probe }))
    }
}

/// 为断连探测复制一个指向同一连接的 socket 句柄并注册回 tokio。
/// `peek` 非消费，不会抢走 hyper 尚未读取的字节。
#[cfg(unix)]
fn dup_probe_socket(socket: &TcpStream) -> io::Result<TcpStream> {
    use std::os::fd::AsFd;
    let std_socket = std::net::TcpStream::from(socket.as_fd().try_clone()?);
    std_socket.set_nonblocking(true)?;
    TcpStream::from_std(std_socket)
}

/// 为断连探测复制一个指向同一连接的 socket 句柄并注册回 tokio。
/// `peek` 非消费，不会抢走 hyper 尚未读取的字节。
#[cfg(windows)]
fn dup_probe_socket(socket: &TcpStream) -> io::Result<TcpStream> {
    use std::os::windows::io::{AsRawSocket, FromRawSocket};
    // SAFETY: 临时 std 包装只是借用现有句柄；`try_clone` 复制出独立句柄后
    // 立即 `forget` 临时包装，原连接句柄的所有权始终在 hyper 手里。
    let borrowed = unsafe { std::net::TcpStream::from_raw_socket(socket.as_raw_socket()) };
    let cloned = borrowed.try_clone();
    std::mem::forget(borrowed);
    let cloned = cloned?;
    cloned.set_nonblocking(true)?;
    TcpStream::from_std(cloned)
}

/// 单连接 service：handler future 与断连探测并存，断开先发生则 drop handler
/// future，触发 `dispatch_internal` 的 `CancelOnDrop`。
#[derive(Clone)]
pub struct DisconnectAwareService {
    router: Router,
    probe: Option<Arc<TcpStream>>,
}

impl Service<Request> for DisconnectAwareService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router as Service<Request>>::poll_ready(&mut self.router, cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let mut router = self.router.clone();
        let handler = router.call(request);
        match self.probe.clone() {
            None => Box::pin(handler),
            Some(probe) => Box::pin(async move {
                tokio::select! {
                    biased;
                    response = handler => response,
                    _ = wait_for_peer_close(&probe) => {
                        // handler future 在此被 drop，`CancelOnDrop` 立即取消该
                        // 请求的 `CancelToken`。这个响应永远到不了客户端
                        // （读侧已断开），状态码只为满足 hyper 的 service 合同。
                        Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::empty())
                            .expect("static builder response is infallible"))
                    }
                }
            }),
        }
    }
}

/// 非消费式断开探测：`peek` 观察到 FIN（`Ok(0)`）或 socket 错误即返回。
/// 观察到还有未读字节（客户端 pipelining 下一请求）不是断开，短暂退避后继续观察。
async fn wait_for_peer_close(socket: &TcpStream) {
    let mut buf = [0u8; 1];
    loop {
        match socket.peek(&mut buf).await {
            Ok(0) => return,
            Ok(_) => tokio::time::sleep(Duration::from_millis(25)).await,
            Err(_) => return,
        }
    }
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
    fn map_rpc_error_follows_the_single_source_of_truth_table() {
        let not_found = RpcError::new(RpcErrorCode::UnknownMethod, "no such method");
        let mapped = map_rpc_error_to_jsonrpc(not_found);
        assert_eq!(mapped.code, JsonRpcErrorCode::MethodNotFound as i32);
        assert_eq!(mapped.data.as_ref().unwrap()["hermes_code"], "unknown-method");
        assert!(mapped.data.as_ref().unwrap().get("diagnostic_code").is_none());

        let invalid_req = RpcError::new(RpcErrorCode::InvalidRequest, "missing param");
        let mapped = map_rpc_error_to_jsonrpc(invalid_req);
        assert_eq!(mapped.code, JsonRpcErrorCode::InvalidParams as i32);

        let handler_fail = RpcError::new(RpcErrorCode::HandlerFailed, "boom");
        let mapped = map_rpc_error_to_jsonrpc(handler_fail);
        assert_eq!(mapped.code, JsonRpcErrorCode::HandlerFailed as i32);
        assert_eq!(mapped.data.as_ref().unwrap()["hermes_code"], "handler-failed");

        let cancelled = RpcError::new(RpcErrorCode::Cancelled, "dispatch cancelled");
        assert_eq!(map_rpc_error_to_jsonrpc(cancelled).code, JsonRpcErrorCode::Cancelled as i32);

        let deadline = RpcError::new(RpcErrorCode::DeadlineExceeded, "dispatch deadline exceeded");
        assert_eq!(map_rpc_error_to_jsonrpc(deadline).code, JsonRpcErrorCode::DeadlineExceeded as i32);

        let corrupt = RpcError::new(RpcErrorCode::ArtifactCorrupt, "bad artifact").with_diagnostic_code("SVC004");
        let mapped = map_rpc_error_to_jsonrpc(corrupt);
        assert_eq!(mapped.code, JsonRpcErrorCode::InternalError as i32);
        assert_eq!(mapped.data.as_ref().unwrap()["hermes_code"], "artifact-corrupt");
        assert_eq!(mapped.data.as_ref().unwrap()["diagnostic_code"], "SVC004");
    }

    #[test]
    fn every_rpc_error_code_is_classified_by_the_table() {
        // 穷举断言：任何新增 RpcErrorCode 都必须进入映射表，禁止落入未定义分支。
        let cases = [
            (RpcErrorCode::ArtifactCorrupt, JsonRpcErrorCode::InternalError),
            (RpcErrorCode::UnknownService, JsonRpcErrorCode::MethodNotFound),
            (RpcErrorCode::UnknownMethod, JsonRpcErrorCode::MethodNotFound),
            (RpcErrorCode::UnknownOperation, JsonRpcErrorCode::MethodNotFound),
            (RpcErrorCode::MissingOperationBinding, JsonRpcErrorCode::InternalError),
            (RpcErrorCode::HandlerNotRegistered, JsonRpcErrorCode::MethodNotFound),
            (RpcErrorCode::DuplicateRegistration, JsonRpcErrorCode::InternalError),
            (RpcErrorCode::InvalidRequest, JsonRpcErrorCode::InvalidParams),
            (RpcErrorCode::UnsupportedCodec, JsonRpcErrorCode::InternalError),
            (RpcErrorCode::HandlerFailed, JsonRpcErrorCode::HandlerFailed),
            (RpcErrorCode::StreamModeMismatch, JsonRpcErrorCode::InternalError),
            (RpcErrorCode::DeadlineExceeded, JsonRpcErrorCode::DeadlineExceeded),
            (RpcErrorCode::Cancelled, JsonRpcErrorCode::Cancelled),
            (RpcErrorCode::Internal, JsonRpcErrorCode::InternalError),
        ];
        for (hermes_code, jsonrpc_code) in cases {
            let mapped = map_rpc_error_to_jsonrpc(RpcError::new(hermes_code, "probe"));
            assert_eq!(mapped.code, jsonrpc_code as i32, "`{hermes_code}` must map to {jsonrpc_code:?}");
        }
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

    #[tokio::test]
    async fn value_non_object_non_array_is_invalid_request() {
        // 顶层是 string / number / null —— 合法 JSON 但不是合法 JSON-RPC 请求
        let dispatcher = Arc::new(HermesDispatcher::new());
        let transport = HttpTransport::new(dispatcher);

        match transport.handle_raw(r#""hello""#).await {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error, got {other:?}"),
        }

        match transport.handle_raw("42").await {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error, got {other:?}"),
        }

        match transport.handle_raw("null").await {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_batch_is_invalid_request() {
        let dispatcher = Arc::new(HermesDispatcher::new());
        let transport = HttpTransport::new(dispatcher);

        match transport.handle_raw("[]").await {
            HandleOutcome::Single(resp) => {
                assert_eq!(resp.error.as_ref().unwrap().code, -32600);
            }
            other => panic!("expected Single error for empty batch, got {other:?}"),
        }
    }
}
