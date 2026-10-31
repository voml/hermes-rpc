//! 真实 HTTP 集成验收 —— 绕过 `handle_raw`，走 axum handler 全链路。
//!
//! 启动真正的 `TcpListener` 绑定 `127.0.0.1:0`，OS 分配随机端口；
//! 用 `tokio::net::TcpStream` 发原生 HTTP/1.1 POST 请求，验证 HTTP 状态码
//! 和 JSON-RPC 错误码。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

use hermes_runtime::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer,
    HermesDispatcher, HermesOperationHandler, MethodIdentityKey,
    MethodStreamMode, OperationContract, OperationEffect, OperationId, OperationIdentityKey,
    OperationKind, OperationParameterContract, OperationParameterKey, OperationReply,
    ResolvedCanonicalType, RpcError, RpcErrorCode, RpcResult, ServiceContract, ServiceIdentityKey, ServiceMethodContract,
};
use hermes_transport_http::{HttpTransport, HttpTransportConfig};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ---------------------------------------------------------------------------
// fixture（和 json_rpc_tests.rs 同一套，tests/ 下各文件独立 crate 所以重复写）
// ---------------------------------------------------------------------------

fn scalar(value: CanonicalScalar) -> ResolvedCanonicalType {
    ResolvedCanonicalType::Scalar(value)
}

fn type_key(ty: &ResolvedCanonicalType) -> String {
    match ty {
        ResolvedCanonicalType::Scalar(value) => value.as_str().to_string(),
        _ => "object".to_string(),
    }
}

fn fixture_dispatcher() -> Arc<HermesDispatcher> {
    fixture_dispatcher_with(Box::new(AddConcatHandler))
}

/// 同一份 `Calc.add` 合同，handler 可替换（断连与 panic 用例用）。
fn fixture_dispatcher_with(handler: Box<dyn HermesOperationHandler>) -> Arc<HermesDispatcher> {
    let add_op = {
        let namespace = vec!["Calc".to_string()];
        let parameter_keys = vec![
            OperationParameterKey::new("a", &type_key(&scalar(CanonicalScalar::Utf8))).unwrap(),
            OperationParameterKey::new("b", &type_key(&scalar(CanonicalScalar::Utf8))).unwrap(),
        ];
        let identity = OperationIdentityKey::new(
            &namespace,
            OperationKind::Query,
            "add",
            parameter_keys,
            "utf8",
        )
        .unwrap();
        OperationContract {
            operation_id: OperationId::derive(&identity),
            kind: OperationKind::Query,
            name: "add".to_string(),
            canonical_path: identity.canonical_path.clone(),
            parameters: vec![
                OperationParameterContract {
                    name: "a".to_string(),
                    canonical_type: scalar(CanonicalScalar::Utf8),
                },
                OperationParameterContract {
                    name: "b".to_string(),
                    canonical_type: scalar(CanonicalScalar::Utf8),
                },
            ],
            cardinality: OperationContract::cardinality_of(&Some(scalar(CanonicalScalar::Utf8))),
            result_type: Some(scalar(CanonicalScalar::Utf8)),
            effect: OperationEffect::Pure,
            capabilities: Vec::new(),
            source_unit_id: 0,
            span: (0..0usize).into(),
            stable_id_hint: None,
        }
    };

    let svc_identity = ServiceIdentityKey::new(vec!["Calc".to_string()]).unwrap();
    let svc_id = hermes_runtime::ServiceId::derive(&svc_identity);
    let method_identity = MethodIdentityKey::new(svc_id.clone(), "add").unwrap();
    let svc = ServiceContract {
        service_id: svc_id,
        name: "Calc".to_string(),
        canonical_path: vec!["Calc".to_string()],
        attributes: Vec::new(),
        methods: vec![ServiceMethodContract {
            method_id: hermes_runtime::MethodId::derive(&method_identity),
            name: "add".to_string(),
            request_type: scalar(CanonicalScalar::Utf8),
            response_type: scalar(CanonicalScalar::Utf8),
            stream: MethodStreamMode::Unary,
            operation_id: add_op.operation_id.clone(),
            attributes: Vec::new(),
            stable_id_hint: None,
        }],
        source_unit_id: 0,
        span: (0..0usize).into(),
        stable_id_hint: None,
    };

    let artifact = ContractArtifact::seal(
        ContractFormat::Hermes,
        ContractProducer::new("http-roundtrip-tests", "0"),
        ContractPayload { operations: vec![add_op.clone()], services: vec![svc] },
    );

    let mut dispatcher = HermesDispatcher::new();
    dispatcher
        .load_contract_artifact(&artifact.to_json().unwrap())
        .unwrap();
    dispatcher
        .register_handler_by_method("Calc", "add", handler)
        .unwrap();

    Arc::new(dispatcher)
}

struct AddConcatHandler;
impl HermesOperationHandler for AddConcatHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _op: &OperationContract,
        params: &Value,
    ) -> RpcResult<OperationReply> {
        let a = params.get("a").and_then(Value::as_str).unwrap_or("");
        let b = params.get("b").and_then(Value::as_str).unwrap_or("");
        Ok(OperationReply::Unary(Value::String(format!("{a}+{b}"))))
    }
}

// ---------------------------------------------------------------------------
// HTTP/1.1 原生客户端辅助
// ---------------------------------------------------------------------------

/// 发起 HTTP/1.1 POST 请求，返回 `(status_code, response_body_string)`
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

    // 读到 Connection: close 关闭
    let mut buf = Vec::with_capacity(1024);
    stream.read_to_end(&mut buf).await.expect("read response");

    let resp = String::from_utf8(buf).expect("utf8 response");

    // 解析状态码
    let status_line = resp.lines().next().unwrap_or("HTTP/1.1 000 Unknown");
    // "HTTP/1.1 200 OK" → 200
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // 跳过 headers，拿 body（第一个空行之后）
    let body_text = resp
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();

    (status_code, body_text)
}

/// 起一个后台 axum serve，返回绑定的 `SocketAddr`
async fn spawn_server(transport: HttpTransport) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, transport.make_service())
            .await
            .expect("axum serve");
    });
    // 给 server 一点时间起来
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    addr
}

// ---------------------------------------------------------------------------
// 测试用例
// ---------------------------------------------------------------------------

#[tokio::test]
async fn real_http_success_roundtrip() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"Calc.add","params":{"a":"hello","b":"world"},"id":"req-1"}"#,
    )
    .await;

    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON response");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["result"], json!("hello+world"));
    assert_eq!(resp["id"], json!("req-1"));
    assert!(resp.get("error").is_none(), "success must not have error field");
}

#[tokio::test]
async fn real_http_method_not_found_returns_200_with_minus_32601() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"Calc.noSuch","params":{},"id":42}"#,
    )
    .await;

    // JSON-RPC 2.0 spec：错误响应 HTTP 仍返回 200
    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON response");
    assert_eq!(resp["error"]["code"], -32601, "method not found → -32601");
    assert_eq!(resp["id"], json!(42));
}

#[tokio::test]
async fn real_http_malformed_json_returns_minus_32700() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(addr, "/rpc", "{ { not valid json").await;

    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON response");
    assert_eq!(resp["error"]["code"], -32700, "parse error → -32700");
}

#[tokio::test]
async fn real_http_notification_returns_204_no_content() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, body) = http_post(
        addr,
        "/rpc",
        r#"{"jsonrpc":"2.0","method":"Calc.add","params":{"a":"x","b":"y"}}"#,
    )
    .await;

    // notification → 204 No Content，没有 body
    assert_eq!(status, 204, "notification must return 204");
    assert!(body.is_empty(), "204 must have no body");
}

#[tokio::test]
async fn real_http_not_found_route_returns_404() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    let (status, _) = http_post(
        addr,
        "/not-the-rpc-endpoint",
        r#"{"jsonrpc":"2.0","method":"Calc.add","params":{},"id":1}"#,
    )
    .await;

    assert_eq!(status, 404);
}

#[tokio::test]
async fn real_http_wrong_method_returns_405() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;

    // 构造一个 GET 请求（handler 只注册了 POST）
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream
        .write_all(b"GET /rpc HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .await
        .expect("write");
    stream.flush().await.expect("flush");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let resp = String::from_utf8_lossy(&buf);
    let status_line = resp.lines().next().unwrap_or("");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert_eq!(status_code, 405, "GET on /rpc must return 405 Method Not Allowed");
}

#[tokio::test]
async fn listen_with_config_bind_addr_works() {
    // 直接验证 listen 方法能正确绑定并启动（不实际 serve 请求，只验证 bind_addr 解析）
    let transport = HttpTransport::with_config(
        fixture_dispatcher(),
        HttpTransportConfig { bind_addr: "127.0.0.1:0".to_string(), request_timeout: None },
    );

    // 在后台起 listen，稍等后 abort
    let handle = transport.serve_background();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    handle.abort();
    // 如果 listen 能正确 bind 并 serve，abort 后应该正常结束
}

// ---------------------------------------------------------------------------
// deadline / 断连取消 / panic —— 跨进程生命周期语义（真实 HTTP 全链路）
// ---------------------------------------------------------------------------

/// 过去的 wire deadline → `-32002`
#[tokio::test]
async fn real_http_past_wire_deadline_returns_minus_32002() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher())).await;
    let deadline_ms = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_millis() as i64 - 1_000;
    let body = format!(r#"{{"jsonrpc":"2.0","method":"Calc.add","params":{{"a":"x","b":"y"}},"id":"dl-1","deadline":{deadline_ms}}}"#);

    let (status, resp) = http_post(addr, "/rpc", &body).await;
    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&resp).expect("valid JSON response");
    assert_eq!(resp["error"]["code"], -32002, "past wire deadline → -32002");
    assert_eq!(resp["error"]["data"]["hermes_code"], "deadline-exceeded");
}

/// 轮询取消令牌的 handler：进入 `execute` 与观察到取消时分别置位标志（断连传播验收用）。
struct CancelObserverHandler {
    started: Arc<AtomicBool>,
    seen: Arc<AtomicBool>,
}

impl HermesOperationHandler for CancelObserverHandler {
    fn execute(
        &self,
        ctx: &hermes_runtime::CallContext,
        _op: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        self.started.store(true, Ordering::SeqCst);
        for _ in 0..500 {
            if ctx.is_cancelled() {
                self.seen.store(true, Ordering::SeqCst);
                return Ok(OperationReply::Unary(Value::String("cancelled".to_string())));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Err(RpcError::new(RpcErrorCode::HandlerFailed, "cancellation was never observed"))
    }
}

/// 客户端在请求进行中断开：transport 的断连探测（连接级 dup socket `peek`）
/// 观察到 FIN/RST → handler future 被 drop → drop guard 取消 runtime
/// `CancelToken` → 阻塞 handler 在下一个检查点观察到取消。
///
/// 测试必须先确认请求真正进入 handler（`started` 置位）再断开：若在服务器
/// 读取请求之前就 drop 客户端 socket，Windows 语义下连接以 RST 结束、请求
/// 从未 dispatch，验收的就不是「进行中断开」而是「从未到达」。
#[tokio::test]
async fn real_http_client_disconnect_cancels_running_handler() {
    let started = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(AtomicBool::new(false));
    let handler = CancelObserverHandler {
        started: Arc::clone(&started),
        seen: Arc::clone(&seen),
    };
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher_with(Box::new(handler)))).await;

    let body = r#"{"jsonrpc":"2.0","method":"Calc.add","params":{"a":"x","b":"y"},"id":"cancel-1"}"#;
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request = format!(
        "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.expect("write request");
    stream.flush().await.expect("flush");

    // 等 handler 真正开始执行，保证断开发生在「请求进行期间」
    for _ in 0..400 {
        if started.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(started.load(Ordering::SeqCst), "request must reach the handler before disconnect");

    // 不读响应直接断开（TcpStream drop 发出 FIN），服务端必须把它传播成取消
    drop(stream);

    for _ in 0..200 {
        if seen.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(seen.load(Ordering::SeqCst), "client disconnect must cancel the running handler via the runtime `CancelToken`");
}

/// 执行时 panic 的 handler（transport 层 panic → `-32603` 验收用）。
struct PanicHandler;

impl HermesOperationHandler for PanicHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _op: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        panic!("http host panic probe");
    }
}

/// handler panic 被 runtime 折算成 `-32603`，宿主继续服务后续请求
#[tokio::test]
async fn real_http_panic_handler_maps_to_minus_32603_and_host_survives() {
    let addr = spawn_server(HttpTransport::new(fixture_dispatcher_with(Box::new(PanicHandler)))).await;

    for probe in 0..2 {
        let (status, resp) = http_post(
            addr,
            "/rpc",
            r#"{"jsonrpc":"2.0","method":"Calc.add","params":{"a":"x","b":"y"},"id":"panic-probe"}"#,
        )
        .await;
        assert_eq!(status, 200, "JSON-RPC error responses still return HTTP 200 (probe {probe})");
        let resp: Value = serde_json::from_str(&resp).expect("valid JSON response");
        assert_eq!(resp["error"]["code"], -32603, "handler panic maps to internal error (probe {probe})");
        assert_eq!(resp["error"]["data"]["hermes_code"], "internal");
    }
}
