//! HTTP/JSON-RPC 2.0 transport adapter 集成测试。
//!
//! 复用 `hermes-runtime` 的 fixture 构造真实 dispatcher，通过 `HttpTransport::handle_raw`
//! 入口覆盖 JSON-RPC 2.0 规范的关键错误码和成功路径。

use std::sync::Arc;

use hermes_runtime::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer,
    HermesDispatcher, HermesOperationHandler, MethodIdentityKey, MethodId,
    MethodStreamMode, OperationContract, OperationEffect, OperationId, OperationIdentityKey,
    OperationKind, OperationParameterContract, OperationParameterKey, OperationReply,
    ResolvedCanonicalType, RpcResult, ServiceContract, ServiceId, ServiceIdentityKey,
    ServiceMethodContract,
};
use hermes_transport_http::{
    JsonRpcErrorCode, JsonRpcRequest, JsonRpcResponse, HttpTransport,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// fixture 构造（仿照 hermes-runtime/tests/main.rs 的模式）
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

fn service_id(path: &[&str]) -> ServiceId {
    let path = path.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    ServiceId::derive(&ServiceIdentityKey::new(path).expect("service key"))
}

fn operation(
    namespace: &[&str],
    name: &str,
    parameters: Vec<(&str, ResolvedCanonicalType)>,
    result_type: Option<ResolvedCanonicalType>,
) -> OperationContract {
    let namespace = namespace.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let parameter_keys = parameters
        .iter()
        .map(|(name, ty)| OperationParameterKey::new(name, &type_key(ty)).expect("param key"))
        .collect::<Vec<_>>();
    let result_key = result_type.as_ref().map(type_key).unwrap_or_else(|| "unit".to_string());
    let identity = OperationIdentityKey::new(
        &namespace,
        OperationKind::Query,
        name,
        parameter_keys,
        &result_key,
    )
    .expect("op identity");
    OperationContract {
        operation_id: OperationId::derive(&identity),
        kind: OperationKind::Query,
        name: name.to_string(),
        canonical_path: identity.canonical_path.clone(),
        parameters: parameters
            .into_iter()
            .map(|(name, canonical_type)| OperationParameterContract { name: name.to_string(), canonical_type })
            .collect(),
        cardinality: OperationContract::cardinality_of(&result_type),
        result_type,
        effect: OperationEffect::Pure,
        capabilities: Vec::new(),
        source_unit_id: 0,
        span: (0..0usize).into(),
    }
}

fn method(
    service_id: &ServiceId,
    name: &str,
    request_type: ResolvedCanonicalType,
    response_type: ResolvedCanonicalType,
    stream: MethodStreamMode,
    operation_id: &OperationId,
) -> ServiceMethodContract {
    let identity = MethodIdentityKey::new(service_id.clone(), name).expect("method key");
    ServiceMethodContract {
        method_id: MethodId::derive(&identity),
        name: name.to_string(),
        request_type,
        response_type,
        stream,
        operation_id: operation_id.clone(),
        attributes: Vec::new(),
    }
}

fn service(path: &[&str], methods: Vec<ServiceMethodContract>) -> ServiceContract {
    let canonical_path = path.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let name = canonical_path.last().cloned().unwrap_or_default();
    ServiceContract {
        service_id: service_id(path),
        name,
        canonical_path,
        attributes: Vec::new(),
        methods,
        source_unit_id: 0,
        span: (0..0usize).into(),
    }
}

fn fixture_dispatcher() -> Arc<HermesDispatcher> {
    // Calc.add：接受两个 utf8 字符串参数，返回一个 utf8 字符串
    let add_op = operation(
        &["Calc"],
        "add",
        vec![("a", scalar(CanonicalScalar::Utf8)), ("b", scalar(CanonicalScalar::Utf8))],
        Some(scalar(CanonicalScalar::Utf8)),
    );

    let svc_id = service_id(&["Calc"]);
    let svc = service(
        &["Calc"],
        vec![method(
            &svc_id,
            "add",
            scalar(CanonicalScalar::Utf8),
            scalar(CanonicalScalar::Utf8),
            MethodStreamMode::Unary,
            &add_op.operation_id,
        )],
    );

    let artifact = ContractArtifact::seal(
        ContractFormat::Hermes,
        ContractProducer::new("hermes-transport-tests", "0"),
        ContractPayload { operations: vec![add_op.clone()], services: vec![svc] },
    );

    let mut dispatcher = HermesDispatcher::new();
    dispatcher
        .load_contract_artifact(&artifact.to_json().expect("seal"))
        .expect("fixture loads");

    // 注册 handler：简单返回 `a + b` 拼接的 utf8 字符串
    dispatcher
        .register_handler_by_method("Calc", "add", Box::new(AddConcatHandler))
        .expect("add handler");

    Arc::new(dispatcher)
}

/// 把 `params.a` 和 `params.b` 拼接返回的测试 handler。
/// a / b 都是 utf8（JSON 字符串），返回也是 utf8。
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
// JsonRpcResponse 断言辅助
// ---------------------------------------------------------------------------

fn assert_success(response: &JsonRpcResponse) {
    assert!(response.error.is_none(), "expected success, got error: {:?}", response.error);
    assert!(response.result.is_some(), "result must be present on success");
    assert_eq!(response.jsonrpc, "2.0");
}

fn assert_error(response: &JsonRpcResponse, expected_code: JsonRpcErrorCode) {
    let err = response.error.as_ref().expect("error must be present");
    assert_eq!(err.code, expected_code as i32);
    assert!(response.result.is_none(), "result must be absent on error");
    assert_eq!(response.jsonrpc, "2.0");
}

// ---------------------------------------------------------------------------
// 测试用例
// ---------------------------------------------------------------------------

/// 方法不存在 → -32601 "Method not found"
/// JSON-RPC 2.0 spec § 3.1：当被请求的方法不存在时
#[test]
fn method_not_found_returns_minus_32601() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = transport.handle_raw(r#"{
        "jsonrpc": "2.0",
        "method": "Calc.noSuchMethod",
        "params": { "foo": "bar" },
        "id": 1
    }"#);

    assert_error(&response, JsonRpcErrorCode::MethodNotFound);
    assert_eq!(response.id, Some(Value::Number(1.into())), "id must be echoed back");
}

/// 完全不存在的 service 也应返回 -32601
#[test]
fn unknown_service_also_returns_minus_32601() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = transport.handle_raw(r#"{
        "jsonrpc": "2.0",
        "method": "NoSuchService.create",
        "params": {},
        "id": 42
    }"#);

    assert_error(&response, JsonRpcErrorCode::MethodNotFound);
}

/// JSON 解析失败 → -32700 "Parse error"
/// JSON-RPC 2.0 spec § 3.1：请求体不是合法 JSON
#[test]
fn malformed_json_returns_minus_32700() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = transport.handle_raw(r#"{ not valid json at all }"#);

    assert_error(&response, JsonRpcErrorCode::ParseError);
    // 解析失败时 id 未知，按规范应为 null
    assert!(response.id.is_none());
}

/// 缺少 jsonrpc 版本字段 → -32600 "Invalid Request"
#[test]
fn missing_jsonrpc_version_returns_minus_32600() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = transport.handle_raw(r#"{
        "method": "Calc.add",
        "params": { "a": "hello", "b": "world" },
        "id": 7
    }"#);

    assert_error(&response, JsonRpcErrorCode::InvalidRequest);
    assert_eq!(response.id, Some(Value::Number(7.into())));
}

/// 错误的版本号 → -32600
#[test]
fn wrong_jsonrpc_version_returns_minus_32600() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = transport.handle_raw(r#"{
        "jsonrpc": "1.0",
        "method": "Calc.add",
        "params": { "a": "x", "b": "y" },
        "id": 3
    }"#);

    assert_error(&response, JsonRpcErrorCode::InvalidRequest);
}

/// method 字段没有点号 → -32600 "Invalid Request"
#[test]
fn malformed_method_string_returns_minus_32600() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = transport.handle_raw(r#"{
        "jsonrpc": "2.0",
        "method": "justOneWord",
        "params": {},
        "id": 9
    }"#);

    assert_error(&response, JsonRpcErrorCode::InvalidRequest);
}

/// 正常端到端：Calc.add 成功
#[test]
fn successful_dispatch_returns_result_and_echoes_id() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = transport.handle_raw(r#"{
        "jsonrpc": "2.0",
        "method": "Calc.add",
        "params": { "a": "hello", "b": "world" },
        "id": "req-123"
    }"#);

    assert_success(&response);
    assert_eq!(response.id, Some(Value::String("req-123".to_string())));

    let result = response.result.unwrap();
    assert_eq!(result, Value::String("hello+world".to_string()));
}

/// 参数校验失败 → -32602 "Invalid params"
#[test]
fn invalid_params_returns_minus_32602() {
    let transport = HttpTransport::new(fixture_dispatcher());

    // a 要求是 UTF8（string），这里传 number
    let response = transport.handle_raw(r#"{
        "jsonrpc": "2.0",
        "method": "Calc.add",
        "params": { "a": 42, "b": "y" },
        "id": 55
    }"#);

    assert_error(&response, JsonRpcErrorCode::InvalidParams);
    assert_eq!(response.id, Some(Value::Number(55.into())));
}

/// 无 id 请求（notification）也能正确返回错误体
#[test]
fn notification_request_with_error_returns_response_with_null_id() {
    let transport = HttpTransport::new(fixture_dispatcher());

    // notification 没有 id，按规范响应 id 应为 null 或省略
    let response = transport.handle_raw(r#"{
        "jsonrpc": "2.0",
        "method": "Calc.noSuch",
        "params": {}
    }"#);

    assert_error(&response, JsonRpcErrorCode::MethodNotFound);
    assert!(response.id.is_none() || response.id == Some(Value::Null));
}

/// HttpTransport 可以直接通过 handle 方法调用
#[test]
fn direct_handle_method_accepts_parsed_request() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let request = JsonRpcRequest {
        jsonrpc: Some("2.0".to_string()),
        method: "Calc.add".to_string(),
        params: json!({ "a": "foo", "b": "bar" }),
        id: Some(Value::Number(99.into())),
    };

    let response = transport.handle(request);
    assert_success(&response);
    assert_eq!(response.result.unwrap(), Value::String("foo+bar".to_string()));
}
