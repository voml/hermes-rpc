//! 基于 Hermes 已有 fixture contract 的真实 transport 集成验收。
//!
//! 流程：加载 `identity_service.contract.json` → `HermesDispatcher::load_contract_artifact`
//! 注册合同 → `register_handler_by_method` 注册 handler → 启动真实 axum serve
//! → 用原生 HTTP/1.1 POST 发 JSON-RPC 请求 → 验证结构化响应。
//!
//! 不构造 mock handler，只验证 Hermes contract → runtime → transport 的全链路打通。

use std::sync::Arc;

use hermes_runtime::{
    HermesDispatcher, HermesOperationHandler, OperationContract, OperationReply, RpcResult,
};
use hermes_transport_http::{HttpTransport, HttpTransportConfig};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ---------------------------------------------------------------------------
// fixture contract + handler
// ---------------------------------------------------------------------------

/// 加载 fixture contract（Hermes generator 测试用的 identity_service）
const CONTRACT_JSON: &str = include_str!("../../hermes-generator/tests/fixtures/identity_service.contract.json");

/// 为 fixture contract 中声明的每个 service.method 注册一个 echo-like handler。
///
/// fixture 包含：
/// - `IdentityService.create`（unary）→ 返回占位 Identity object
/// - `IdentityService.lookup`（server-stream）→ 返回单个 Identity 元素
fn fixture_dispatcher() -> Arc<HermesDispatcher> {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher
        .load_contract_artifact(CONTRACT_JSON)
        .expect("fixture contract must load cleanly");

    // IdentityService.create — unary handler
    dispatcher
        .register_handler_by_method("IdentityService", "create", Box::new(EchoCreateHandler))
        .expect("create handler must register");

    // IdentityService.lookup — server-stream handler
    dispatcher
        .register_handler_by_method("IdentityService", "lookup", Box::new(EchoLookupHandler))
        .expect("lookup handler must register");

    Arc::new(dispatcher)
}

struct EchoCreateHandler;
impl HermesOperationHandler for EchoCreateHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _op: &OperationContract,
        params: &Value,
    ) -> RpcResult<OperationReply> {
        // fixture 的 resultType 是 user Identity —— WireCodec::Object，只要是 JSON object 即可
        Ok(OperationReply::Unary(json!({
            "id": "new-id-1",
            "echo": params,
        })))
    }
}

struct EchoLookupHandler;
impl HermesOperationHandler for EchoLookupHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _op: &OperationContract,
        params: &Value,
    ) -> RpcResult<OperationReply> {
        // server-stream，responseType 是 Optional<user Identity>
        // stream 每个元素按 Optional 校验：object 或 null 都可以
        Ok(OperationReply::ServerStream(vec![json!({
            "found": true,
            "echo": params,
        })]))
    }
}

// ---------------------------------------------------------------------------
// HTTP/1.1 原生客户端辅助（和 http_roundtrip.rs 同构，避免引入 reqwest 重依赖）
// ---------------------------------------------------------------------------

async fn http_post(addr: std::net::SocketAddr, path: &str, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
         Content-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        path = path,
        addr = addr,
        len = body.len(),
        body = body,
    );
    stream.write_all(request.as_bytes()).await.expect("write request");
    stream.flush().await.expect("flush");

    let mut buf = Vec::with_capacity(1024);
    stream.read_to_end(&mut buf).await.expect("read response");

    let resp = String::from_utf8(buf).expect("utf8 response");

    let status_line = resp.lines().next().unwrap_or("HTTP/1.1 000 Unknown");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let body_text = resp
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();

    (status_code, body_text)
}

/// 起一个后台 axum serve，返回绑定的 `SocketAddr`
async fn spawn_server(transport: HttpTransport) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, transport.router())
            .await
            .expect("axum serve");
    });
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    addr
}

// ---------------------------------------------------------------------------
// 验收测试
// ---------------------------------------------------------------------------

/// 从 fixture contract 加载 IdentityService 并成功调用 create。
#[tokio::test]
async fn fixture_identity_service_create_roundtrip() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"IdentityService.create","params":{"request":{"name":"alice"}},"id":42}"#,
    )
    .await;

    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(resp["error"].is_null(), "unexpected error: {:?}", resp["error"]);
    // result 必须是 object（匹配 fixture 的 WireCodec::Object）
    assert!(resp["result"].is_object(), "result must be object, got: {}", resp["result"]);
    assert_eq!(resp["result"]["id"], "new-id-1");
    assert_eq!(resp["id"], json!(42));
}

/// 调用 fixture 的 server-stream 方法 lookup —— handler 返回的 stream 元素数组
/// 直接作为 JSON 数组在 wire 上传输（骨架阶段不做真正的 streaming）。
#[tokio::test]
async fn fixture_identity_service_lookup_stream_roundtrip() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"IdentityService.lookup","params":{"id":"123e4567-e89b-12d3-a456-426614174000"},"id":7}"#,
    )
    .await;

    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert!(resp["error"].is_null());
    // stream → JSON 数组
    assert!(resp["result"].is_array(), "stream result must be JSON array");
    assert_eq!(resp["result"].as_array().unwrap().len(), 1);
    assert_eq!(resp["result"][0]["found"], true);
    assert_eq!(resp["id"], json!(7));
}

/// fixture 里不存在的 method 返回 -32601（Method not found）。
/// 这验证了 fixture contract 注册后 transport 的 method-not-found 路径仍然正确。
#[tokio::test]
async fn fixture_unknown_method_returns_minus_32601() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"IdentityService.noSuchMethod","params":{},"id":99}"#,
    )
    .await;

    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(resp["error"]["code"], -32601);
    assert_eq!(resp["id"], json!(99));
}

/// fixture 里完全不存在的 service 也返回 -32601。
#[tokio::test]
async fn fixture_unknown_service_returns_minus_32601() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"TotallyMissing.doThing","params":{},"id":1}"#,
    )
    .await;

    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(resp["error"]["code"], -32601);
}

/// fixture 方法的参数 shape 不符 → -32602 Invalid params。
/// create 的 request 参数必须是 object（WireCodec::Object），传 string 应被拒绝。
#[tokio::test]
async fn fixture_create_invalid_param_shape_returns_minus_32602() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"IdentityService.create","params":{"request":"not-an-object"},"id":5}"#,
    )
    .await;

    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(resp["error"]["code"], -32602, "object shape mismatch → -32602");
}

/// listen + serve_background 完整启动路径仍能工作（被 graceful shutdown 增强后）。
#[tokio::test]
async fn fixture_listen_via_serve_background() {
    let transport = HttpTransport::new(fixture_dispatcher());
    let handle = transport.serve_background(HttpTransportConfig::default());

    // 稍等 server 起来
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // 简单验证：abort 后能干净退出（不 hang）
    handle.abort();
    let _ = handle.await;
}
