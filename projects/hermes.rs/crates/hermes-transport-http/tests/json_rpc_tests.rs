//! HTTP/JSON-RPC 2.0 transport adapter 集成测试。
//!
//! 复用 `hermes-runtime` 的 fixture 构造真实 dispatcher，通过 `HttpTransport::handle_raw`
//! 入口覆盖 JSON-RPC 2.0 规范的关键错误码和成功路径（含 batch / notification）。

use std::sync::Arc;

use hermes_runtime::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, HermesDispatcher,
    HermesOperationHandler, MethodId, MethodIdentityKey, MethodStreamMode, OperationContract, OperationEffect, OperationId,
    OperationIdentityKey, OperationKind, OperationParameterContract, OperationParameterKey, OperationReply,
    ResolvedCanonicalType, RpcResult, ServiceContract, ServiceId, ServiceIdentityKey, ServiceMethodContract,
};
use hermes_transport_http::{HandleOutcome, HttpTransport, JsonRpcErrorCode, JsonRpcRequest, JsonRpcResponse};
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
    let identity =
        OperationIdentityKey::new(&namespace, OperationKind::Query, name, parameter_keys, &result_key).expect("op identity");
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
    dispatcher.load_contract_artifact(&artifact.to_json().expect("seal")).expect("fixture loads");

    // 注册 handler：简单返回 `a + b` 拼接的 utf8 字符串
    dispatcher.register_handler_by_method("Calc", "add", Box::new(AddConcatHandler)).expect("add handler");

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
// HandleOutcome 断言辅助
// ---------------------------------------------------------------------------

fn expect_single(outcome: HandleOutcome) -> JsonRpcResponse {
    match outcome {
        HandleOutcome::Single(resp) => resp,
        other => panic!("expected HandleOutcome::Single, got {other:?}"),
    }
}

fn expect_batch(outcome: HandleOutcome) -> Vec<JsonRpcResponse> {
    match outcome {
        HandleOutcome::Batch(vec) => vec,
        other => panic!("expected HandleOutcome::Batch, got {other:?}"),
    }
}

fn expect_no_content(outcome: HandleOutcome) {
    match outcome {
        HandleOutcome::NoContent => {}
        other => panic!("expected HandleOutcome::NoContent, got {other:?}"),
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

    let response = expect_single(transport.handle_raw(
        r#"{
        "jsonrpc": "2.0",
        "method": "Calc.noSuchMethod",
        "params": { "foo": "bar" },
        "id": 1
    }"#,
    ));

    assert_error(&response, JsonRpcErrorCode::MethodNotFound);
    assert_eq!(response.id, Some(Value::Number(1.into())), "id must be echoed back");
}

/// 完全不存在的 service 也应返回 -32601
#[test]
fn unknown_service_also_returns_minus_32601() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = expect_single(transport.handle_raw(
        r#"{
        "jsonrpc": "2.0",
        "method": "NoSuchService.create",
        "params": {},
        "id": 42
    }"#,
    ));

    assert_error(&response, JsonRpcErrorCode::MethodNotFound);
}

/// JSON 解析失败 → -32700 "Parse error"
/// JSON-RPC 2.0 spec § 3.1：请求体不是合法 JSON
#[test]
fn malformed_json_returns_minus_32700() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = expect_single(transport.handle_raw(r#"{ not valid json at all }"#));

    assert_error(&response, JsonRpcErrorCode::ParseError);
    // 解析失败时 id 未知，按规范应为 null
    assert!(response.id.is_none());
}

/// 缺少 jsonrpc 版本字段 → -32600 "Invalid Request"
#[test]
fn missing_jsonrpc_version_returns_minus_32600() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = expect_single(transport.handle_raw(
        r#"{
        "method": "Calc.add",
        "params": { "a": "hello", "b": "world" },
        "id": 7
    }"#,
    ));

    assert_error(&response, JsonRpcErrorCode::InvalidRequest);
    assert_eq!(response.id, Some(Value::Number(7.into())));
}

/// 错误的版本号 → -32600
#[test]
fn wrong_jsonrpc_version_returns_minus_32600() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = expect_single(transport.handle_raw(
        r#"{
        "jsonrpc": "1.0",
        "method": "Calc.add",
        "params": { "a": "x", "b": "y" },
        "id": 3
    }"#,
    ));

    assert_error(&response, JsonRpcErrorCode::InvalidRequest);
}

/// method 字段没有点号 → -32600 "Invalid Request"
#[test]
fn malformed_method_string_returns_minus_32600() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = expect_single(transport.handle_raw(
        r#"{
        "jsonrpc": "2.0",
        "method": "justOneWord",
        "params": {},
        "id": 9
    }"#,
    ));

    assert_error(&response, JsonRpcErrorCode::InvalidRequest);
}

/// 正常端到端：Calc.add 成功
#[test]
fn successful_dispatch_returns_result_and_echoes_id() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = expect_single(transport.handle_raw(
        r#"{
        "jsonrpc": "2.0",
        "method": "Calc.add",
        "params": { "a": "hello", "b": "world" },
        "id": "req-123"
    }"#,
    ));

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
    let response = expect_single(transport.handle_raw(
        r#"{
        "jsonrpc": "2.0",
        "method": "Calc.add",
        "params": { "a": 42, "b": "y" },
        "id": 55
    }"#,
    ));

    assert_error(&response, JsonRpcErrorCode::InvalidParams);
    assert_eq!(response.id, Some(Value::Number(55.into())));
}

/// notification 请求（无 `id`）无论 dispatch 成功失败都不返回响应
/// JSON-RPC 2.0 spec §2.0：notification 没有响应
#[test]
fn notification_request_returns_no_content() {
    let transport = HttpTransport::new(fixture_dispatcher());

    // notification 没有 id → NoContent（即使方法不存在也不返回错误体）
    expect_no_content(transport.handle_raw(
        r#"{
        "jsonrpc": "2.0",
        "method": "Calc.noSuch",
        "params": {}
    }"#,
    ));

    // 即使方法存在也是 NoContent
    expect_no_content(transport.handle_raw(
        r#"{
        "jsonrpc": "2.0",
        "method": "Calc.add",
        "params": { "a": "x", "b": "y" }
    }"#,
    ));
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

// ---------------------------------------------------------------------------
// batch 测试
// ---------------------------------------------------------------------------

/// 空 batch `[]` → -32600 Invalid Request
#[test]
fn empty_batch_returns_minus_32600() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let response = expect_single(transport.handle_raw("[]"));
    assert_error(&response, JsonRpcErrorCode::InvalidRequest);
}

/// batch 两个正常请求 → 返回两个响应
#[test]
fn batch_with_two_valid_requests_returns_two_responses() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let batch = expect_batch(transport.handle_raw(
        r#"[
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "x", "b": "y"}, "id": 1},
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "foo", "b": "bar"}, "id": 2}
    ]"#,
    ));

    assert_eq!(batch.len(), 2);
    assert_success(&batch[0]);
    assert_eq!(batch[0].result.as_ref().unwrap(), &Value::String("x+y".to_string()));
    assert_eq!(batch[0].id, Some(Value::Number(1.into())));
    assert_success(&batch[1]);
    assert_eq!(batch[1].result.as_ref().unwrap(), &Value::String("foo+bar".to_string()));
    assert_eq!(batch[1].id, Some(Value::Number(2.into())));
}

/// batch 混合成功和失败
#[test]
fn batch_mixed_success_and_error() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let batch = expect_batch(transport.handle_raw(
        r#"[
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "x", "b": "y"}, "id": 1},
        {"jsonrpc": "2.0", "method": "Calc.noSuch", "params": {}, "id": 2}
    ]"#,
    ));

    assert_eq!(batch.len(), 2);
    assert_success(&batch[0]);
    assert_error(&batch[1], JsonRpcErrorCode::MethodNotFound);
}

/// batch 中 notification 被跳过，不进入响应数组
#[test]
fn batch_with_notification_excludes_it_from_response_array() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let batch = expect_batch(transport.handle_raw(
        r#"[
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "x", "b": "y"}, "id": 1},
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "skip", "b": "me"}},
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "a", "b": "b"}, "id": 3}
    ]"#,
    ));

    // notification 被排除了 → 只有 2 个响应
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].id, Some(Value::Number(1.into())));
    assert_eq!(batch[1].id, Some(Value::Number(3.into())));
}

/// batch 全是 notification → NoContent
#[test]
fn batch_all_notifications_returns_no_content() {
    let transport = HttpTransport::new(fixture_dispatcher());

    expect_no_content(transport.handle_raw(
        r#"[
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "x", "b": "y"}},
        {"jsonrpc": "2.0", "method": "Calc.noSuch", "params": {}}
    ]"#,
    ));
}

/// batch 元素不是合法 JSON-RPC request → 该元素单独返回 -32600
#[test]
fn batch_invalid_element_gets_own_error() {
    let transport = HttpTransport::new(fixture_dispatcher());

    let batch = expect_batch(transport.handle_raw(
        r#"[
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "x", "b": "y"}, "id": 1},
        "not a json rpc object",
        {"jsonrpc": "2.0", "method": "Calc.add", "params": {"a": "a", "b": "b"}, "id": 2}
    ]"#,
    ));

    assert_eq!(batch.len(), 3);
    assert_success(&batch[0]);
    // 中间那个非法元素 → InvalidRequest
    assert_error(&batch[1], JsonRpcErrorCode::InvalidRequest);
    assert_success(&batch[2]);
}
