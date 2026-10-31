//! AOT 生成物目标宿主示例：VOS service artifact → method 级 adapter →
//! `hermes-transport-http` 真实 HTTP 宿主。
//!
//! 流程与 `tests/aot_roundtrip.rs` 的闭环一致，可直接运行观察：
//!
//! 1. 严格加载 `vos/hermes.contract/v0` artifact；
//! 2. `generate_service` 生成 method 级 TS adapter，`write_to` 落盘；
//! 3. 读回生成物 `routes.ts`，宿主按其中的 `serviceName.methodName` 与 stream
//!    形态注册 handler（生成物即挂载清单，wire 名不硬编码）；
//! 4. `HttpTransport::listen` 起真实 server（含断连探测与 graceful shutdown），
//!    POST `http://<bind>/rpc` 即可调用，Ctrl+C 退出。
//!
//! ```sh
//! cargo run -p hermes-transport-http --example aot_http_host -- [out_dir] [bind_addr]
//! ```

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use hermes_generator::{AotTarget, generate_service};
use hermes_runtime::{
    ContractArtifact, HermesDispatcher, HermesOperationHandler, OperationContract, OperationReply, RpcResult,
};
use hermes_transport_http::{HttpTransport, HttpTransportConfig};
use serde_json::{Value, json};

const CONTRACT_JSON: &str = include_str!("../../hermes-generator/tests/fixtures/identity_service.contract.json");

/// 生成物 route 表的宿主侧投影（`routes.ts` 格式由生成器拥有、逐行确定）。
struct GeneratedRoute {
    service_name: String,
    method_name: String,
    stream: String,
}

fn parse_route_table(ts: &str) -> Vec<GeneratedRoute> {
    let mut routes = Vec::new();
    let mut in_table = false;
    let mut service_name: Option<String> = None;
    let mut method_name = String::new();
    let mut stream = String::new();
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
        if trimmed.starts_with('"') {
            service_name = Some(String::new());
            continue;
        }
        if trimmed == "}," {
            routes.push(GeneratedRoute {
                service_name: service_name.take().expect("route entry in progress"),
                method_name,
                stream,
            });
            method_name = String::new();
            stream = String::new();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("serviceName: ") {
            *service_name.as_mut().expect("inside a route entry") = unquote(value);
        } else if let Some(value) = trimmed.strip_prefix("methodName: ") {
            method_name = unquote(value);
        } else if let Some(value) = trimmed.strip_prefix("stream: ") {
            stream = unquote(value);
        }
    }
    routes
}

fn unquote(field: &str) -> String {
    field.trim_end_matches(',').trim_matches('"').to_string()
}

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = args.first().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("aot-out"));
    let bind_addr = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let artifact = ContractArtifact::from_json(CONTRACT_JSON).map_err(|err| format!("artifact: {}", err.message))?;
    let plan = generate_service(&artifact, AotTarget::Desktop)?;
    plan.write_to(&out_dir)?;
    println!("generated method-level adapter into {}", out_dir.display());
    for file in plan.files() {
        println!("  {}", file.path());
    }

    let routes_ts = fs::read_to_string(out_dir.join("hermes/service/routes.ts"))?;
    let routes = parse_route_table(&routes_ts);

    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_contract_artifact(CONTRACT_JSON)?;
    for route in &routes {
        let handler: Box<dyn HermesOperationHandler> = match route.stream.as_str() {
            "unary" => Box::new(UnaryWriteEcho),
            "server-stream" => Box::new(StreamReadEcho),
            other => return Err(format!("unexpected stream mode `{other}` in generated route table").into()),
        };
        dispatcher.register_handler_by_method(&route.service_name, &route.method_name, handler)?;
        println!("mounted {}.{}/{}", route.service_name, route.method_name, route.stream);
    }

    let transport = HttpTransport::with_config(
        Arc::new(dispatcher),
        HttpTransportConfig { bind_addr, request_timeout: Some(std::time::Duration::from_secs(30)) },
    );
    println!("serving JSON-RPC at /rpc — Ctrl+C to stop");
    transport.listen().await?;
    Ok(())
}
