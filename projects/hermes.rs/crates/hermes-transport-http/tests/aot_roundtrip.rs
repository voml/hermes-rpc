//! C 块闭环：VOS service artifact → `hermes-generator` method 级 adapter →
//! `hermes-transport-http` 宿主挂载 → 真实 HTTP 请求验收读 / 写 / 错误路径。
//!
//! 流程：
//!
//! 1. `ContractArtifact::from_json` 严格加载 `vos/hermes.contract/v0` fixture；
//! 2. `generate_service` 生成 method 级 TS adapter（`hermes/service/` 六个文件），
//!    `write_to` 落盘后读回 `routes.ts` 与 `metadata.ts`；
//! 3. 断言生成物携带的 durable identity（route key、`methodId`、`operationId`、
//!    fingerprint）与 artifact payload 逐项一致——生成器与 runtime 只认同一份身份；
//! 4. 宿主从**生成物 route 表**解析出 `serviceName.methodName` 与 stream 形态，
//!    据此注册 handler 并拼 wire method 名（不是硬编码）；
//! 5. 真实 HTTP/JSON-RPC 请求验收读路径（`lookup`，server-stream）、写路径
//!    （`create`，unary）与错误路径（未知 method `-32601`、参数形态不符 `-32602`）。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use hermes_generator::{AotTarget, generate_service};
use hermes_runtime::{
    ContractArtifact, HermesDispatcher, HermesOperationHandler, MethodStreamMode, OperationContract, OperationReply,
    RpcResult,
};
use hermes_transport_http::HttpTransport;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const CONTRACT_JSON: &str = include_str!("../../hermes-generator/tests/fixtures/identity_service.contract.json");

// ---------------------------------------------------------------------------
// 生成物 route 表解析（`routes.ts` 的格式由生成器拥有、逐行确定）
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedRoute {
    route_key: String,
    service_id: String,
    service_name: String,
    method_name: String,
    method_id: String,
    operation_id: String,
    stream: String,
}

fn parse_route_table(ts: &str) -> Vec<ParsedRoute> {
    let mut routes = Vec::new();
    let mut in_table = false;
    let mut current: Option<ParsedRoute> = None;
    for line in ts.lines() {
        let trimmed = line.trim();
        if trimmed == "const routeTable = {" {
            in_table = true;
            continue;
        }
        if !in_table {
            continue;
        }
        if trimmed == "} as const;" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix('"') {
            let route_key = rest.split("\": {").next().expect("route key line").to_string();
            current = Some(ParsedRoute {
                route_key,
                service_id: String::new(),
                service_name: String::new(),
                method_name: String::new(),
                method_id: String::new(),
                operation_id: String::new(),
                stream: String::new(),
            });
            continue;
        }
        if trimmed == "}," {
            routes.push(current.take().expect("route entry in progress"));
            continue;
        }
        if let Some(route) = current.as_mut() {
            if let Some(value) = trimmed.strip_prefix("serviceId: ") {
                route.service_id = unquote(value);
            } else if let Some(value) = trimmed.strip_prefix("serviceName: ") {
                route.service_name = unquote(value);
            } else if let Some(value) = trimmed.strip_prefix("methodName: ") {
                route.method_name = unquote(value);
            } else if let Some(value) = trimmed.strip_prefix("methodId: ") {
                route.method_id = unquote(value);
            } else if let Some(value) = trimmed.strip_prefix("operationId: ") {
                route.operation_id = unquote(value);
            } else if let Some(value) = trimmed.strip_prefix("stream: ") {
                route.stream = unquote(value);
            }
        }
    }
    routes
}

fn unquote(field: &str) -> String {
    field.trim_end_matches(',').trim_matches('"').to_string()
}

fn stream_mode_name(stream: MethodStreamMode) -> &'static str {
    match stream {
        MethodStreamMode::Unary => "unary",
        MethodStreamMode::ServerStream => "server-stream",
    }
}

fn temp_out_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("hermes-aot-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

// ---------------------------------------------------------------------------
// 业务 handler：写路径（unary）与读路径（server-stream）
// ---------------------------------------------------------------------------

struct UnaryWriteEcho;

impl HermesOperationHandler for UnaryWriteEcho {
    fn execute(&self, _ctx: &hermes_runtime::CallContext, _op: &OperationContract, params: &Value) -> RpcResult<OperationReply> {
        Ok(OperationReply::Unary(json!({ "recorded": true, "echo": params })))
    }
}

struct StreamReadEcho;

impl HermesOperationHandler for StreamReadEcho {
    fn execute(&self, _ctx: &hermes_runtime::CallContext, _op: &OperationContract, params: &Value) -> RpcResult<OperationReply> {
        Ok(OperationReply::ServerStream(vec![json!({ "found": true, "echo": params })]))
    }
}

// ---------------------------------------------------------------------------
// HTTP/1.1 原生客户端辅助（与其它 transport 测试同构，不引入 reqwest）
// ---------------------------------------------------------------------------

async fn http_post(addr: std::net::SocketAddr, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request = format!(
        "POST /rpc HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.expect("write request");
    stream.flush().await.expect("flush");

    let mut buf = Vec::with_capacity(1024);
    stream.read_to_end(&mut buf).await.expect("read response");
    let resp = String::from_utf8(buf).expect("utf8 response");

    let status: u16 = resp
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let body_text = resp.split_once("\r\n\r\n").map(|(_, body)| body.to_string()).unwrap_or_default();
    (status, body_text)
}

async fn spawn_server(transport: HttpTransport) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, transport.make_service()).await.expect("axum serve");
    });
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    addr
}

// ---------------------------------------------------------------------------
// 验收
// ---------------------------------------------------------------------------

/// 主闭环：artifact → 生成 adapter → 宿主按生成物 route 表挂载 → 真实 HTTP 读 / 写 / 错误。
#[tokio::test]
async fn aot_generated_adapter_mounts_http_host_and_serves_read_write_error() {
    let artifact = ContractArtifact::from_json(CONTRACT_JSON).expect("fixture artifact must load strictly");

    // 1. method 级 AOT 生成并落盘
    let plan = generate_service(&artifact, AotTarget::Desktop).expect("fixture artifact must generate cleanly");
    let out = temp_out_root("roundtrip");
    let report = plan.write_to(&out).expect("write_to must succeed");
    assert_eq!(report.written().len(), 6, "five generated files plus one template");

    // 2. 读回生成物，断言 durable identity 与 artifact payload 逐项一致
    let routes_ts = fs::read_to_string(out.join("hermes/service/routes.ts")).expect("routes.ts on disk");
    let metadata_ts = fs::read_to_string(out.join("hermes/service/metadata.ts")).expect("metadata.ts on disk");
    assert!(
        metadata_ts.contains(&format!("export const contractFingerprint = \"{}\";", artifact.fingerprint)),
        "metadata must carry the artifact fingerprint"
    );

    let routes = parse_route_table(&routes_ts);
    let payload_routes: Vec<ParsedRoute> = artifact
        .payload
        .services
        .iter()
        .flat_map(|service| {
            service.methods.iter().map(move |method| ParsedRoute {
                route_key: format!("{}/{}", service.service_id, method.name),
                service_id: service.service_id.to_string(),
                service_name: service.name.clone(),
                method_name: method.name.clone(),
                method_id: method.method_id.to_string(),
                operation_id: method.operation_id.to_string(),
                stream: stream_mode_name(method.stream).to_string(),
            })
        })
        .collect();
    assert_eq!(routes, payload_routes, "generated route table must mirror the artifact identities exactly");

    // 3. 宿主从生成物 route 表挂载 handler 与 wire method 名
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_contract_artifact(CONTRACT_JSON).expect("fixture contract must load into registry");
    for route in &routes {
        let handler: Box<dyn HermesOperationHandler> = match route.stream.as_str() {
            "unary" => Box::new(UnaryWriteEcho),
            "server-stream" => Box::new(StreamReadEcho),
            other => panic!("unexpected stream mode `{other}` in generated route table"),
        };
        dispatcher
            .register_handler_by_method(&route.service_name, &route.method_name, handler)
            .unwrap_or_else(|err| panic!("mount `{}.{}` from generated table failed: {err}", route.service_name, route.method_name));
    }
    let addr = spawn_server(HttpTransport::new(Arc::new(dispatcher))).await;

    let wire_name = |route: &ParsedRoute| format!("{}.{}", route.service_name, route.method_name);
    let write_route = routes.iter().find(|route| route.stream == "unary").expect("fixture carries a unary route");
    let read_route = routes.iter().find(|route| route.stream == "server-stream").expect("fixture carries a server-stream route");

    // 4a. 写路径：create（unary）→ 200 + object result
    let create_body = format!(
        r#"{{"jsonrpc":"2.0","method":"{}","params":{{"request":{{"name":"alice"}}}},"id":"aot-write"}}"#,
        wire_name(write_route)
    );
    let (status, body) = http_post(addr, &create_body).await;
    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert!(resp["error"].is_null(), "write path must succeed, got {:?}", resp["error"]);
    assert_eq!(resp["result"]["recorded"], true);
    assert_eq!(resp["result"]["echo"]["request"]["name"], "alice");
    assert_eq!(resp["id"], json!("aot-write"));

    // 4b. 读路径：lookup（server-stream）→ 200 + JSON 数组 result
    let lookup_body = format!(
        r#"{{"jsonrpc":"2.0","method":"{}","params":{{"id":"123e4567-e89b-12d3-a456-426614174000"}},"id":"aot-read"}}"#,
        wire_name(read_route)
    );
    let (status, body) = http_post(addr, &lookup_body).await;
    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert!(resp["error"].is_null(), "read path must succeed, got {:?}", resp["error"]);
    let elements = resp["result"].as_array().expect("server-stream result must be a JSON array");
    assert_eq!(elements.len(), 1);
    assert_eq!(elements[0]["found"], true);
    assert_eq!(elements[0]["echo"]["id"], "123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(resp["id"], json!("aot-read"));

    // 4c. 错误路径：生成表之外的 method → -32601
    let unknown_body = format!(
        r#"{{"jsonrpc":"2.0","method":"{}.absentMethod","params":{{}},"id":"aot-unknown"}}"#,
        write_route.service_name
    );
    let (status, body) = http_post(addr, &unknown_body).await;
    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(resp["error"]["code"], -32601, "method outside the generated table must be -32601");

    // 4d. 错误路径：参数形态不符 → -32602
    let invalid_body = format!(
        r#"{{"jsonrpc":"2.0","method":"{}","params":{{"request":"not-an-object"}},"id":"aot-invalid"}}"#,
        wire_name(write_route)
    );
    let (status, body) = http_post(addr, &invalid_body).await;
    assert_eq!(status, 200);
    let resp: Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(resp["error"]["code"], -32602, "object param sent as string must be -32602");

    let _ = fs::remove_dir_all(&out);
}

/// 重复生成的文件所有权语义：`Generated` 内容一致记 unchanged，`Template` 已存在记 skipped。
#[test]
fn aot_plan_write_to_respects_file_ownership_on_regen() {
    let artifact = ContractArtifact::from_json(CONTRACT_JSON).expect("fixture artifact must load strictly");
    let plan = generate_service(&artifact, AotTarget::Desktop).expect("fixture artifact must generate cleanly");
    let out = temp_out_root("ownership");

    let first = plan.write_to(&out).expect("first write");
    assert_eq!(first.written().len(), 6);
    assert!(first.unchanged().is_empty() && first.skipped().is_empty());

    let second = plan.write_to(&out).expect("second write");
    assert_eq!(second.written().len(), 0, "regeneration must not rewrite bytes");
    assert_eq!(second.unchanged().len(), 5, "five `Generated` files compare equal");
    assert_eq!(second.skipped().len(), 1, "the `handlers.ts` template is never overwritten");

    let _ = fs::remove_dir_all(&out);
}
