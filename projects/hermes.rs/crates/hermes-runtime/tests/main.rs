//! Hermes RPC 集成测试：上游合同 fixture → registry → dispatch。
//!
//! fixture 直接取自上游 `vos-language` 的合同参考样例（`vos/iris.contract/v0`），
//! 测试先按上游类型解码再重新封装成 `vos/hermes.contract/v0`，证明 Hermes 没有
//! 合同语义副本：payload、fingerprint 与 identity 都来自 `vos-types`。
//!
//! 失败路径覆盖：未知版本 / 未知字段 / 指纹不匹配 / producer 缺版本 / payload
//! 内部诊断码保留，以及 registry 冲突、类型不符与 stream 模式不符。

use std::{
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use hermes_runtime::{
    AddHandler, CallOptions, CancelToken, CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer,
    ContractRegistry, DispatchReply, EchoHandler, HermesDispatchRequest, HermesDispatchResponse, HermesDispatcher,
    HermesOperationHandler, MethodId, MethodIdentityKey, MethodStreamMode, OperationContract, OperationEffect, OperationId,
    OperationIdentityKey, OperationKind, OperationParameterContract, OperationParameterKey, OperationReply,
    ResolvedCanonicalType, RpcErrorCode, RpcResult, ServiceContract, ServiceId, ServiceIdentityKey, ServiceMethodContract,
    parse_contract_artifact, seal_hermes_artifact,
};
use serde_json::{Value, json};

/// 上游 `vos-language` 合同参考 fixture（Iris 格式）。
const UPSTREAM_FIXTURE: &str = include_str!("fixtures/operations_services.contract.json");

const CREATE_OP: &str = "op_38ecee3afe0deee18f4f4c89f7b33c390d84844a3bf0832632f2f612cd132437";
const LOOKUP_OP: &str = "op_c9d4e9ea491ec269e6681aa00b353d24269e48702ae07081cd65cea635361d5d";
const CREATE_METHOD: &str = "mtd_a1195cc42bc0418387f233d5db60833493a2aecab3923e75c97be3509f0aecfd";
const LOOKUP_METHOD: &str = "mtd_8209e030c55458c829b98b16cd5cb582db22fca4ec29fcc991241dfc36ce3ff8";
const USER_ID: &str = "018f2c00-0000-7000-8000-000000000001";
const UNKNOWN_OP: &str = "op_0000000000000000000000000000000000000000000000000000000000000000";

// ---------------------------------------------------------------------------
// fixture 与合同构造
// ---------------------------------------------------------------------------

fn upstream_artifact() -> ContractArtifact {
    serde_json::from_str(UPSTREAM_FIXTURE).expect("upstream fixture decodes as a contract artifact")
}

fn upstream_value() -> Value {
    serde_json::from_str(UPSTREAM_FIXTURE).expect("upstream fixture decodes as JSON")
}

fn hermes_artifact() -> ContractArtifact {
    let upstream = upstream_artifact();
    assert_eq!(upstream.version, ContractFormat::Iris, "upstream fixture stays an Iris artifact");
    seal_hermes_artifact(&upstream)
}

fn hermes_fixture_json() -> String {
    hermes_artifact().to_json().expect("Hermes artifact serializes")
}

fn scalar(value: CanonicalScalar) -> ResolvedCanonicalType {
    ResolvedCanonicalType::Scalar(value)
}

fn type_key(ty: &ResolvedCanonicalType) -> String {
    match ty {
        ResolvedCanonicalType::Scalar(value) => value.as_str().to_string(),
        ResolvedCanonicalType::User { path, type_id } => format!("{}#{type_id}", path.join("::")),
        ResolvedCanonicalType::Reference(inner) => format!("&{}", type_key(inner)),
        ResolvedCanonicalType::Optional(inner) => format!("{}?", type_key(inner)),
        ResolvedCanonicalType::List(inner) => format!("[{}]", type_key(inner)),
        ResolvedCanonicalType::Generic { path, .. } => path.join("::"),
    }
}

fn operation(
    namespace: &[&str],
    name: &str,
    parameters: Vec<(&str, ResolvedCanonicalType)>,
    result_type: Option<ResolvedCanonicalType>,
) -> OperationContract {
    let namespace = namespace.iter().map(|segment| segment.to_string()).collect::<Vec<_>>();
    let parameter_keys = parameters
        .iter()
        .map(|(name, ty)| OperationParameterKey::new(name, &type_key(ty)).expect("parameter key"))
        .collect::<Vec<_>>();
    let result_key = result_type.as_ref().map(type_key).unwrap_or_else(|| "unit".to_string());
    let identity = OperationIdentityKey::new(&namespace, OperationKind::Query, name, parameter_keys, &result_key)
        .expect("operation identity key");
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
    let identity = MethodIdentityKey::new(service_id.clone(), name).expect("method identity key");
    ServiceMethodContract {
        method_id: MethodId::derive(&identity),
        name: name.to_string(),
        request_type,
        response_type,
        stream,
        operation_id: operation_id.clone(),
    }
}

fn service(path: &[&str], methods: Vec<ServiceMethodContract>) -> ServiceContract {
    let canonical_path = path.iter().map(|segment| segment.to_string()).collect::<Vec<_>>();
    let name = canonical_path.last().cloned().unwrap_or_default();
    ServiceContract {
        service_id: ServiceId::derive(&ServiceIdentityKey::new(canonical_path.clone()).expect("service key")),
        name,
        canonical_path,
        methods,
        source_unit_id: 0,
        span: (0..0usize).into(),
    }
}

fn artifact(operations: Vec<OperationContract>, services: Vec<ServiceContract>) -> ContractArtifact {
    ContractArtifact::seal(
        ContractFormat::Hermes,
        ContractProducer::new("hermes-tests", "0"),
        ContractPayload { operations, services },
    )
}

// ---------------------------------------------------------------------------
// handler stubs
// ---------------------------------------------------------------------------

struct CreateHandler;

impl HermesOperationHandler for CreateHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        Ok(OperationReply::Unary(json!({ "id": USER_ID, "displayName": "Ada" })))
    }
}

struct LookupHandler;

impl HermesOperationHandler for LookupHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        Ok(OperationReply::ServerStream(vec![json!({ "id": USER_ID }), Value::Null]))
    }
}

struct WrongCreateReply;

impl HermesOperationHandler for WrongCreateReply {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        Ok(OperationReply::Unary(json!("not-an-object")))
    }
}

struct StreamOnUnary;

impl HermesOperationHandler for StreamOnUnary {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        Ok(OperationReply::ServerStream(vec![json!({ "id": USER_ID })]))
    }
}

struct UnaryOnStream;

impl HermesOperationHandler for UnaryOnStream {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        Ok(OperationReply::Unary(json!({ "id": USER_ID })))
    }
}

/// sleep 越过 deadline 的 handler。
struct SlowHandler {
    delay: Duration,
}

impl HermesOperationHandler for SlowHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        thread::sleep(self.delay);
        Ok(OperationReply::Unary(json!({ "id": USER_ID })))
    }
}

/// 主动取消自身调用的 handler。
struct CancelHandler;

impl HermesOperationHandler for CancelHandler {
    fn execute(
        &self,
        ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        if let Some(token) = ctx.token() {
            token.cancel();
        }
        Ok(OperationReply::Unary(json!({ "id": USER_ID })))
    }
}

/// 占住并发许可直到 gate 放行的 handler。
struct HoldHandler {
    started: mpsc::Sender<()>,
    gate: Arc<(Mutex<bool>, Condvar)>,
}

impl HermesOperationHandler for HoldHandler {
    fn execute(
        &self,
        _ctx: &hermes_runtime::CallContext,
        _operation: &OperationContract,
        _params: &Value,
    ) -> RpcResult<OperationReply> {
        let _ = self.started.send(());
        let (lock, condvar) = &*self.gate;
        let mut open = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        while !*open {
            open = condvar.wait(open).unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        Ok(OperationReply::Unary(json!({ "id": USER_ID })))
    }
}

fn release_gate(gate: &Arc<(Mutex<bool>, Condvar)>) {
    let (lock, condvar) = &**gate;
    *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    condvar.notify_all();
}

fn dispatcher_with_handlers() -> HermesDispatcher {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_contract_artifact(&hermes_fixture_json()).expect("Hermes fixture loads");
    dispatcher.register_handler(CREATE_OP, Box::new(CreateHandler)).expect("create handler");
    dispatcher.register_handler(LOOKUP_OP, Box::new(LookupHandler)).expect("lookup handler");
    dispatcher
}

fn create_params() -> Value {
    json!({ "request": { "email": "ada@example.com" } })
}

fn lookup_params() -> Value {
    json!({ "id": USER_ID })
}

// ---------------------------------------------------------------------------
// envelope：格式、版本、指纹与诊断码
// ---------------------------------------------------------------------------

#[test]
fn upstream_fixture_round_trips_through_hermes_envelope() {
    let upstream = upstream_artifact();
    let hermes = seal_hermes_artifact(&upstream);

    assert_eq!(hermes.fingerprint, upstream.fingerprint, "fingerprint is derived by upstream, not Hermes");
    assert_eq!(hermes.version, ContractFormat::Hermes);

    let reparsed = parse_contract_artifact(&hermes.to_json().expect("serialize")).expect("Hermes artifact loads");
    assert_eq!(reparsed, hermes);
    assert_eq!(reparsed.payload, upstream.payload, "payload is field-for-field the upstream payload");
}

#[test]
fn loader_accepts_only_the_hermes_contract_format() {
    let err = parse_contract_artifact(UPSTREAM_FIXTURE).expect_err("Iris artifact is rejected by Hermes");
    assert_eq!(err.code(), RpcErrorCode::ArtifactCorrupt);
    assert_eq!(err.diagnostic_code(), Some("ART010"));
    assert!(err.message().contains("vos/hermes.contract/v0"));
}

#[test]
fn loader_rejects_unknown_contract_version() {
    let mut value = upstream_value();
    value["version"] = json!("vos/hermes.contract/v1");
    let err = parse_contract_artifact(&value.to_string()).expect_err("unknown version rejected");
    assert_eq!(err.diagnostic_code(), Some("ART010"));
}

#[test]
fn loader_rejects_unknown_fields() {
    let mut value = upstream_value();
    value["version"] = json!("vos/hermes.contract/v0");
    value["futureField"] = json!(true);
    let err = parse_contract_artifact(&value.to_string()).expect_err("unknown field rejected");
    assert_eq!(err.diagnostic_code(), Some("ART010"));
}

fn hermes_value() -> Value {
    let mut value = upstream_value();
    value["version"] = json!("vos/hermes.contract/v0");
    value
}

#[test]
fn loader_rejects_fingerprint_mismatch() {
    let mut value = hermes_value();
    value["fingerprint"] = json!("0".repeat(64));
    let err = parse_contract_artifact(&value.to_string()).expect_err("fingerprint mismatch rejected");
    assert_eq!(err.code(), RpcErrorCode::ArtifactCorrupt);
    assert_eq!(err.diagnostic_code(), Some("ART020"));
}

#[test]
fn loader_rejects_missing_producer_version() {
    let mut value = hermes_value();
    value["producer"]["version"] = json!("");
    let err = parse_contract_artifact(&value.to_string()).expect_err("producer version required");
    assert_eq!(err.diagnostic_code(), Some("ART021"));
}

#[test]
fn loader_preserves_payload_diagnostic_codes() {
    let mut payload: ContractPayload = serde_json::from_value(upstream_value()["payload"].clone()).expect("payload decodes");
    payload.services[0].service_id =
        ServiceId::parse(&format!("svc_{}", "0".repeat(64))).expect("well-formed but mismatched service id");
    let artifact = ContractArtifact::seal(ContractFormat::Hermes, ContractProducer::new("fixture", "0"), payload);

    let err = parse_contract_artifact(&artifact.to_json().expect("seal")).expect_err("mismatched service id rejected");
    assert_eq!(err.code(), RpcErrorCode::ArtifactCorrupt);
    assert_eq!(err.diagnostic_code(), Some("SVC013"), "payload diagnostics keep their upstream code");
}

// ---------------------------------------------------------------------------
// registry：解析与冲突检测
// ---------------------------------------------------------------------------

#[test]
fn registry_resolves_service_method_to_method_and_operation_identity() {
    let mut registry = ContractRegistry::new();
    registry.register_artifact(&hermes_artifact()).expect("fixture registers");

    assert_eq!(registry.service_count(), 1);
    assert_eq!(registry.operation_count(), 2);
    assert_eq!(registry.service_names(), vec!["IdentityService"]);

    let create = registry.resolve("IdentityService", "create").expect("create resolves");
    assert_eq!(create.method_id().as_str(), CREATE_METHOD);
    assert_eq!(create.operation_id().as_str(), CREATE_OP);
    assert_eq!(create.stream(), MethodStreamMode::Unary);

    let lookup = registry.resolve("IdentityService", "lookup").expect("lookup resolves");
    assert_eq!(lookup.method_id().as_str(), LOOKUP_METHOD);
    assert_eq!(lookup.operation_id().as_str(), LOOKUP_OP);
    assert_eq!(lookup.stream(), MethodStreamMode::ServerStream);
}

#[test]
fn registry_lookup_failures_are_structured_and_never_guess() {
    let mut registry = ContractRegistry::new();
    registry.register_artifact(&hermes_artifact()).expect("fixture registers");

    let service = registry.resolve("IdentityService", "missing").expect_err("unknown method");
    assert_eq!(service.code(), RpcErrorCode::UnknownMethod);

    let unknown = registry.resolve("NoSuchService", "create").expect_err("unknown service");
    assert_eq!(unknown.code(), RpcErrorCode::UnknownService);
}

#[test]
fn registry_rejects_duplicate_service_name() {
    let op_a = operation(&["a"], "go", vec![], Some(scalar(CanonicalScalar::Utf8)));
    let op_b = operation(&["b"], "go", vec![], Some(scalar(CanonicalScalar::Utf8)));
    let service_a = service(
        &["a", "Echo"],
        vec![method(
            &service_id(&["a", "Echo"]),
            "go",
            scalar(CanonicalScalar::Utf8),
            scalar(CanonicalScalar::Utf8),
            MethodStreamMode::Unary,
            &op_a.operation_id,
        )],
    );
    let service_b = service(
        &["b", "Echo"],
        vec![method(
            &service_id(&["b", "Echo"]),
            "go",
            scalar(CanonicalScalar::Utf8),
            scalar(CanonicalScalar::Utf8),
            MethodStreamMode::Unary,
            &op_b.operation_id,
        )],
    );

    let mut registry = ContractRegistry::new();
    let err = registry
        .register_artifact(&artifact(vec![op_a, op_b], vec![service_a, service_b]))
        .expect_err("duplicate service name rejected");
    assert_eq!(err.code(), RpcErrorCode::DuplicateRegistration);
    assert!(err.message().contains("service name `Echo`"));
}

#[test]
fn registry_rejects_duplicate_method_name() {
    let op_a = operation(&["a"], "first", vec![], None);
    let op_b = operation(&["a"], "second", vec![], None);
    let owner = service_id(&["a", "Echo"]);
    let svc = service(
        &["a", "Echo"],
        vec![
            method(
                &owner,
                "go",
                scalar(CanonicalScalar::Utf8),
                scalar(CanonicalScalar::Utf8),
                MethodStreamMode::Unary,
                &op_a.operation_id,
            ),
            method(
                &owner,
                "go",
                scalar(CanonicalScalar::Utf8),
                scalar(CanonicalScalar::Utf8),
                MethodStreamMode::Unary,
                &op_b.operation_id,
            ),
        ],
    );

    let mut registry = ContractRegistry::new();
    let err = registry.register_artifact(&artifact(vec![op_a, op_b], vec![svc])).expect_err("duplicate method name rejected");
    assert_eq!(err.code(), RpcErrorCode::DuplicateRegistration);
    assert!(err.message().contains("method name `Echo.go`"));
}

#[test]
fn registry_rejects_method_bound_to_missing_operation() {
    let owner = service_id(&["a", "Echo"]);
    let missing = OperationId::parse(UNKNOWN_OP).expect("well-formed unknown id");
    let svc = service(
        &["a", "Echo"],
        vec![method(
            &owner,
            "go",
            scalar(CanonicalScalar::Utf8),
            scalar(CanonicalScalar::Utf8),
            MethodStreamMode::Unary,
            &missing,
        )],
    );

    let mut registry = ContractRegistry::new();
    let err = registry.register_artifact(&artifact(vec![], vec![svc])).expect_err("missing operation rejected");
    assert_eq!(err.code(), RpcErrorCode::MissingOperationBinding);
}

#[test]
fn registry_rejects_duplicate_registration_of_the_same_artifact() {
    let mut registry = ContractRegistry::new();
    registry.register_artifact(&hermes_artifact()).expect("first register");
    let err = registry.register_artifact(&hermes_artifact()).expect_err("second register rejected");
    assert_eq!(err.code(), RpcErrorCode::DuplicateRegistration);
}

#[test]
fn registry_rejects_non_hermes_artifacts() {
    let mut registry = ContractRegistry::new();
    let err = registry.register_artifact(&upstream_artifact()).expect_err("Iris artifact rejected");
    assert_eq!(err.code(), RpcErrorCode::ArtifactCorrupt);
    assert_eq!(err.diagnostic_code(), Some("ART010"));
}

#[test]
fn registry_rejects_stale_fingerprint_at_mount_time_and_stays_empty() {
    let mut artifact = hermes_artifact();
    artifact.fingerprint = "0".repeat(64);

    let mut registry = ContractRegistry::new();
    let err = registry.register_artifact(&artifact).expect_err("stale fingerprint must fail at mount time");
    assert_eq!(err.code(), RpcErrorCode::ArtifactCorrupt);
    assert_eq!(err.diagnostic_code(), Some("ART020"));
    assert_eq!(registry.service_count(), 0, "a rejected artifact must not leave a partial registry");
    assert_eq!(registry.operation_count(), 0, "a rejected artifact must not leave a partial registry");
}

#[test]
fn registry_rejects_identity_that_disagrees_with_its_canonical_path_at_mount_time() {
    let mut artifact = hermes_artifact();
    artifact.payload.services[0].service_id =
        ServiceId::parse(&format!("svc_{}", "0".repeat(64))).expect("well-formed but mismatched service id");
    // 重新 seal 让 fingerprint 与损坏后的 payload 一致，专门验证 payload 本身
    // 也会在挂载期被上游校验拒绝，而不是只靠 fingerprint 挡住。
    let artifact = ContractArtifact::seal(ContractFormat::Hermes, artifact.producer, artifact.payload);

    let mut registry = ContractRegistry::new();
    let err = registry.register_artifact(&artifact).expect_err("corrupt payload must fail at mount time");
    assert_eq!(err.code(), RpcErrorCode::ArtifactCorrupt);
    assert_eq!(err.diagnostic_code(), Some("SVC013"));
    assert_eq!(registry.service_count(), 0, "a rejected artifact must not leave a partial registry");
    assert_eq!(registry.operation_count(), 0, "a rejected artifact must not leave a partial registry");
}

#[test]
fn registry_rejects_a_second_artifact_reusing_a_service_route_at_mount_time() {
    fn mountable(path: &[&str], operation_name: &str) -> ContractArtifact {
        let op = operation(path, operation_name, vec![], Some(scalar(CanonicalScalar::Utf8)));
        let owner = service_id(path);
        let svc = service(
            path,
            vec![method(
                &owner,
                "go",
                scalar(CanonicalScalar::Utf8),
                scalar(CanonicalScalar::Utf8),
                MethodStreamMode::Unary,
                &op.operation_id,
            )],
        );
        artifact(vec![op], vec![svc])
    }

    let mut registry = ContractRegistry::new();
    registry.register_artifact(&mountable(&["a", "Echo"], "go")).expect("first mount succeeds");

    let err = registry
        .register_artifact(&mountable(&["b", "Echo"], "go"))
        .expect_err("the same service route must not be mounted twice");
    assert_eq!(err.code(), RpcErrorCode::DuplicateRegistration);
    assert!(err.message().contains("service name `Echo`"));
    assert_eq!(registry.service_count(), 1, "the rejected mount must not change the registry");
}

fn service_id(path: &[&str]) -> ServiceId {
    let path = path.iter().map(|segment| segment.to_string()).collect::<Vec<_>>();
    ServiceId::derive(&ServiceIdentityKey::new(path).expect("service identity key"))
}

// ---------------------------------------------------------------------------
// dispatch：unary / server-stream 与类型校验
// ---------------------------------------------------------------------------

#[test]
fn dispatch_routes_unary_and_server_stream_from_the_upstream_fixture() {
    let dispatcher = dispatcher_with_handlers();

    let unary = dispatcher.dispatch("IdentityService", "create", &create_params()).expect("unary dispatch");
    assert_eq!(unary, DispatchReply::Unary(json!({ "id": USER_ID, "displayName": "Ada" })));

    let stream = dispatcher.dispatch("IdentityService", "lookup", &lookup_params()).expect("stream dispatch");
    assert_eq!(stream, DispatchReply::ServerStream(vec![json!({ "id": USER_ID }), Value::Null]));
}

#[test]
fn dispatch_request_returns_wire_envelope_with_resolved_identities() {
    let dispatcher = dispatcher_with_handlers();
    let request = HermesDispatchRequest::new("IdentityService", "create", create_params());

    let response = dispatcher.dispatch_request(&request);
    assert_eq!(response.service(), "IdentityService");
    assert_eq!(response.method(), "create");
    assert_eq!(response.operation_id(), Some(CREATE_OP));
    assert_eq!(response.method_id(), Some(CREATE_METHOD));
    assert_eq!(response.result(), Some(&json!({ "id": USER_ID, "displayName": "Ada" })));
    assert_eq!(response.payload_count(), 1);

    let roundtrip: HermesDispatchResponse =
        serde_json::from_str(&serde_json::to_string(&response).expect("serialize")).expect("deserialize");
    assert_eq!(roundtrip, response);

    let error = dispatcher.dispatch_request(&HermesDispatchRequest::new("Nope", "create", json!({})));
    assert!(error.operation_id().is_none());
    assert_eq!(error.error().expect("error").code(), RpcErrorCode::UnknownService);
    let wire = serde_json::to_value(&error).expect("serialize error envelope");
    assert_eq!(wire["error"]["code"], json!("unknown-service"));
    assert!(wire.get("result").is_none());
}

#[test]
fn dispatch_rejects_mistyped_missing_and_extra_parameters() {
    let dispatcher = dispatcher_with_handlers();

    let mistyped = dispatcher.dispatch("IdentityService", "lookup", &json!({ "id": 7 })).expect_err("uuid is a string");
    assert_eq!(mistyped.code(), RpcErrorCode::InvalidRequest);
    assert!(mistyped.message().contains("must be JSON string"));

    let missing = dispatcher.dispatch("IdentityService", "lookup", &json!({})).expect_err("missing param");
    assert_eq!(missing.code(), RpcErrorCode::InvalidRequest);
    assert!(missing.message().contains("missing parameter `id`"));

    let extra =
        dispatcher.dispatch("IdentityService", "lookup", &json!({ "id": USER_ID, "extra": 1 })).expect_err("extra param");
    assert_eq!(extra.code(), RpcErrorCode::InvalidRequest);
    assert!(extra.message().contains("unknown parameter `extra`"));

    let not_object = dispatcher.dispatch("IdentityService", "create", &json!([])).expect_err("params must be object");
    assert_eq!(not_object.code(), RpcErrorCode::InvalidRequest);
    assert!(not_object.message().contains("params must be a JSON object"));
}

#[test]
fn dispatch_rejects_handler_reply_that_does_not_match_the_declared_result_type() {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_contract_artifact(&hermes_fixture_json()).expect("fixture loads");
    dispatcher.register_handler(CREATE_OP, Box::new(WrongCreateReply)).expect("handler");

    let err = dispatcher.dispatch("IdentityService", "create", &create_params()).expect_err("result type mismatch");
    assert_eq!(err.code(), RpcErrorCode::HandlerFailed);
    assert!(err.message().contains("must be JSON object"));
}

#[test]
fn dispatch_rejects_stream_mode_mismatch_in_both_directions() {
    let mut unary_service = HermesDispatcher::new();
    unary_service.load_contract_artifact(&hermes_fixture_json()).expect("fixture loads");
    unary_service.register_handler(CREATE_OP, Box::new(StreamOnUnary)).expect("handler");
    let err = unary_service.dispatch("IdentityService", "create", &create_params()).expect_err("stream on unary");
    assert_eq!(err.code(), RpcErrorCode::StreamModeMismatch);
    assert!(err.message().contains("is unary but the handler returned a server stream"));

    let mut stream_service = HermesDispatcher::new();
    stream_service.load_contract_artifact(&hermes_fixture_json()).expect("fixture loads");
    stream_service.register_handler(LOOKUP_OP, Box::new(UnaryOnStream)).expect("handler");
    let err = stream_service.dispatch("IdentityService", "lookup", &lookup_params()).expect_err("unary on stream");
    assert_eq!(err.code(), RpcErrorCode::StreamModeMismatch);
    assert!(err.message().contains("is server-stream but the handler returned one value"));
}

#[test]
fn dispatch_rejects_missing_handler_and_malformed_handler_registration() {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_contract_artifact(&hermes_fixture_json()).expect("fixture loads");
    dispatcher.register_handler(CREATE_OP, Box::new(CreateHandler)).expect("handler");

    let err = dispatcher.dispatch("IdentityService", "lookup", &lookup_params()).expect_err("no lookup handler");
    assert_eq!(err.code(), RpcErrorCode::HandlerNotRegistered);

    let malformed = dispatcher.register_handler("op_short", Box::new(EchoHandler)).expect_err("malformed id");
    assert_eq!(malformed.code(), RpcErrorCode::InvalidRequest);

    let unknown = dispatcher.register_handler(UNKNOWN_OP, Box::new(EchoHandler)).expect_err("unknown operation");
    assert_eq!(unknown.code(), RpcErrorCode::UnknownOperation);
}

#[test]
fn dispatch_rejects_generic_types_without_a_wire_codec() {
    let boxed = ResolvedCanonicalType::Generic {
        path: vec!["Box".to_string()],
        arguments: vec![hermes_runtime::ResolvedCanonicalTypeArgument::Type(scalar(CanonicalScalar::Utf8))],
    };
    let op = operation(&["a"], "reject", vec![("value", boxed.clone())], Some(scalar(CanonicalScalar::Utf8)));
    let owner = service_id(&["a", "Reject"]);
    let svc = service(
        &["a", "Reject"],
        vec![method(&owner, "reject", boxed, scalar(CanonicalScalar::Utf8), MethodStreamMode::Unary, &op.operation_id)],
    );

    let mut registry = ContractRegistry::new();
    registry.register_artifact(&artifact(vec![op.clone()], vec![svc])).expect("contract registers");
    let mut dispatcher = HermesDispatcher::with_registry(registry);
    dispatcher.register_handler(op.operation_id.as_str(), Box::new(EchoHandler)).expect("handler");

    let err = dispatcher.dispatch("Reject", "reject", &json!({ "value": "x" })).expect_err("generic rejected");
    assert_eq!(err.code(), RpcErrorCode::UnsupportedCodec);
    assert!(err.message().contains("unsupported codec for type `Box<utf8>`"));
}

#[test]
fn unsupported_codec_nested_inside_list_or_optional_is_not_downgraded() {
    let boxed = ResolvedCanonicalType::Generic {
        path: vec!["Box".to_string()],
        arguments: vec![hermes_runtime::ResolvedCanonicalTypeArgument::Type(scalar(CanonicalScalar::Utf8))],
    };
    let nested = ResolvedCanonicalType::List(Box::new(ResolvedCanonicalType::Optional(Box::new(boxed))));
    let op = operation(&["a"], "reject_nested", vec![("values", nested.clone())], Some(scalar(CanonicalScalar::Utf8)));
    let owner = service_id(&["a", "RejectNested"]);
    let svc = service(
        &["a", "RejectNested"],
        vec![method(&owner, "rejectNested", nested, scalar(CanonicalScalar::Utf8), MethodStreamMode::Unary, &op.operation_id)],
    );

    let mut registry = ContractRegistry::new();
    registry.register_artifact(&artifact(vec![op.clone()], vec![svc])).expect("contract registers");
    let mut dispatcher = HermesDispatcher::with_registry(registry);
    dispatcher.register_handler(op.operation_id.as_str(), Box::new(EchoHandler)).expect("handler");

    let err =
        dispatcher.dispatch("RejectNested", "rejectNested", &json!({ "values": ["x"] })).expect_err("nested generic rejected");
    assert_eq!(err.code(), RpcErrorCode::UnsupportedCodec, "unknown codecs must never degrade to `unknown`");
    assert!(err.message().contains("unsupported codec for type `Box<utf8>`"));
}

#[test]
fn unsupported_return_codec_surfaces_as_unsupported_codec_not_handler_failed() {
    let boxed = ResolvedCanonicalType::Generic {
        path: vec!["Box".to_string()],
        arguments: vec![hermes_runtime::ResolvedCanonicalTypeArgument::Type(scalar(CanonicalScalar::Utf8))],
    };
    let op = operation(&["a"], "reject_return", vec![], Some(boxed.clone()));
    let owner = service_id(&["a", "RejectReturn"]);
    let svc = service(
        &["a", "RejectReturn"],
        vec![method(&owner, "rejectReturn", scalar(CanonicalScalar::Utf8), boxed, MethodStreamMode::Unary, &op.operation_id)],
    );

    let mut registry = ContractRegistry::new();
    registry.register_artifact(&artifact(vec![op.clone()], vec![svc])).expect("contract registers");
    let mut dispatcher = HermesDispatcher::with_registry(registry);
    dispatcher.register_handler(op.operation_id.as_str(), Box::new(EchoHandler)).expect("handler");

    let err = dispatcher.dispatch("RejectReturn", "rejectReturn", &json!({})).expect_err("unmapped return type rejected");
    assert_eq!(err.code(), RpcErrorCode::UnsupportedCodec, "the declared return type, not the value shape, is unsupported");
}

#[test]
fn scalar_codec_table_covers_all_nineteen_typed_variants() {
    let mapped = CanonicalScalar::ALL
        .into_iter()
        .map(|scalar| (scalar, hermes_runtime::codec_for(&ResolvedCanonicalType::Scalar(scalar)).expect("scalar codec")))
        .collect::<Vec<_>>();
    assert_eq!(mapped.len(), 19);
    assert_eq!(hermes_runtime::codec_for(&scalar(CanonicalScalar::Utf8)).unwrap(), hermes_runtime::WireCodec::Utf8);
    assert_eq!(hermes_runtime::codec_for(&scalar(CanonicalScalar::Utf16)).unwrap(), hermes_runtime::WireCodec::Utf16);
    assert_eq!(hermes_runtime::codec_for(&scalar(CanonicalScalar::I64)).unwrap(), hermes_runtime::WireCodec::Int64);
    assert_eq!(hermes_runtime::codec_for(&scalar(CanonicalScalar::U64)).unwrap(), hermes_runtime::WireCodec::Uint64);
    assert_eq!(hermes_runtime::codec_for(&scalar(CanonicalScalar::Decimal)).unwrap(), hermes_runtime::WireCodec::Decimal);
    assert_eq!(
        hermes_runtime::codec_for(&scalar(CanonicalScalar::I32)).unwrap(),
        hermes_runtime::WireCodec::Integer { min: i64::from(i32::MIN), max: i64::from(i32::MAX) }
    );
    assert_ne!(
        hermes_runtime::codec_for(&scalar(CanonicalScalar::I32)).unwrap(),
        hermes_runtime::codec_for(&scalar(CanonicalScalar::I64)).unwrap()
    );
}

#[test]
fn add_handler_uses_contract_parameter_types() {
    let op_a = operation(
        &["a"],
        "add",
        vec![("a", scalar(CanonicalScalar::I64)), ("b", scalar(CanonicalScalar::I64))],
        Some(scalar(CanonicalScalar::I64)),
    );
    let owner = service_id(&["a", "Calc"]);
    let svc = service(
        &["a", "Calc"],
        vec![method(
            &owner,
            "add",
            scalar(CanonicalScalar::I64),
            scalar(CanonicalScalar::I64),
            MethodStreamMode::Unary,
            &op_a.operation_id,
        )],
    );

    let mut registry = ContractRegistry::new();
    registry.register_artifact(&artifact(vec![op_a.clone()], vec![svc])).expect("contract registers");
    let mut dispatcher = HermesDispatcher::with_registry(registry);
    dispatcher.register_handler(op_a.operation_id.as_str(), Box::new(AddHandler)).expect("handler");

    let reply = dispatcher.dispatch("Calc", "add", &json!({ "a": "3", "b": "4" })).expect("add dispatch");
    assert_eq!(reply, DispatchReply::Unary(json!("7")));

    let err = dispatcher.dispatch("Calc", "add", &json!({ "a": 3, "b": "4" })).expect_err("i64 must be a string");
    assert_eq!(err.code(), RpcErrorCode::InvalidRequest);
}

// ---------------------------------------------------------------------------
// lifecycle：deadline、取消与并发上限
// ---------------------------------------------------------------------------

fn lifecycle_dispatcher(handler: Box<dyn HermesOperationHandler>) -> HermesDispatcher {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_contract_artifact(&hermes_fixture_json()).expect("fixture loads");
    dispatcher.register_handler(CREATE_OP, handler).expect("handler");
    dispatcher
}

#[test]
fn dispatch_with_expired_deadline_returns_deadline_exceeded() {
    let dispatcher = lifecycle_dispatcher(Box::new(CreateHandler));
    let options = CallOptions::with_deadline(Instant::now() - Duration::from_millis(1));

    let err = dispatcher.dispatch_with("IdentityService", "create", &create_params(), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DeadlineExceeded);
}

#[test]
fn dispatch_times_out_when_handler_outlives_the_deadline() {
    let dispatcher = lifecycle_dispatcher(Box::new(SlowHandler { delay: Duration::from_millis(80) }));
    let options = CallOptions::with_timeout(Duration::from_millis(10));

    let err = dispatcher.dispatch_with("IdentityService", "create", &create_params(), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DeadlineExceeded);
}

#[test]
fn dispatch_with_cancelled_token_and_handler_initiated_cancel_return_cancelled() {
    let dispatcher = lifecycle_dispatcher(Box::new(CreateHandler));
    let token = CancelToken::new();
    token.cancel();
    let options = CallOptions::new().and_cancellation(token);
    let err = dispatcher.dispatch_with("IdentityService", "create", &create_params(), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::Cancelled);

    let dispatcher = lifecycle_dispatcher(Box::new(CancelHandler));
    let options = CallOptions::new().and_cancellation(CancelToken::new());
    let err = dispatcher.dispatch_with("IdentityService", "create", &create_params(), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::Cancelled);
}

#[test]
fn concurrency_limit_queues_until_the_permit_is_released() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (started, started_rx) = mpsc::channel();

    let mut dispatcher = lifecycle_dispatcher(Box::new(HoldHandler { started, gate: gate.clone() }));
    dispatcher.set_max_concurrency(Some(1));
    assert_eq!(dispatcher.max_concurrency(), Some(1));

    let dispatcher = Arc::new(dispatcher);
    let background = {
        let dispatcher = Arc::clone(&dispatcher);
        thread::spawn(move || dispatcher.dispatch("IdentityService", "create", &create_params()))
    };
    started_rx.recv_timeout(Duration::from_secs(5)).expect("handler started");

    let options = CallOptions::with_timeout(Duration::from_millis(30));
    let err = dispatcher
        .dispatch_with("IdentityService", "create", &create_params(), &options)
        .expect_err("second call waits for the permit");
    assert_eq!(err.code(), RpcErrorCode::DeadlineExceeded);

    release_gate(&gate);
    let held = background.join().expect("background joins").expect("held dispatch succeeds");
    assert!(matches!(held, DispatchReply::Unary(_)));

    let options = CallOptions::with_timeout(Duration::from_millis(500));
    dispatcher.dispatch_with("IdentityService", "create", &create_params(), &options).expect("permit released");
}

#[test]
fn cancellation_while_waiting_for_a_permit_returns_cancelled() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (started, started_rx) = mpsc::channel();

    let mut dispatcher = lifecycle_dispatcher(Box::new(HoldHandler { started, gate: gate.clone() }));
    dispatcher.set_max_concurrency(Some(1));

    let dispatcher = Arc::new(dispatcher);
    let background = {
        let dispatcher = Arc::clone(&dispatcher);
        thread::spawn(move || dispatcher.dispatch("IdentityService", "create", &create_params()))
    };
    started_rx.recv_timeout(Duration::from_secs(5)).expect("handler started");

    let token = CancelToken::new();
    let canceller = token.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        canceller.cancel();
    });

    let options = CallOptions::new().and_timeout(Duration::from_secs(5)).and_cancellation(token);
    let err = dispatcher
        .dispatch_with("IdentityService", "create", &create_params(), &options)
        .expect_err("queued call must observe cancellation, not wait for the permit");
    assert_eq!(err.code(), RpcErrorCode::Cancelled, "waiting for a permit must stay cancellable");

    cancel_thread.join().expect("cancel thread joins");
    release_gate(&gate);
    let held = background.join().expect("background joins").expect("held dispatch succeeds");
    assert!(matches!(held, DispatchReply::Unary(_)));
}

// ---------------------------------------------------------------------------
// wire 错误码
// ---------------------------------------------------------------------------

#[test]
fn rpc_error_code_wire_names_are_kebab_case() {
    let codes = [
        (RpcErrorCode::ArtifactCorrupt, "artifact-corrupt"),
        (RpcErrorCode::UnknownService, "unknown-service"),
        (RpcErrorCode::UnknownMethod, "unknown-method"),
        (RpcErrorCode::UnknownOperation, "unknown-operation"),
        (RpcErrorCode::MissingOperationBinding, "missing-operation-binding"),
        (RpcErrorCode::HandlerNotRegistered, "handler-not-registered"),
        (RpcErrorCode::DuplicateRegistration, "duplicate-registration"),
        (RpcErrorCode::InvalidRequest, "invalid-request"),
        (RpcErrorCode::UnsupportedCodec, "unsupported-codec"),
        (RpcErrorCode::HandlerFailed, "handler-failed"),
        (RpcErrorCode::StreamModeMismatch, "stream-mode-mismatch"),
        (RpcErrorCode::DeadlineExceeded, "deadline-exceeded"),
        (RpcErrorCode::Cancelled, "cancelled"),
        (RpcErrorCode::Internal, "internal"),
    ];
    for (code, expected) in codes {
        assert_eq!(code.as_str(), expected);
        assert_eq!(serde_json::to_string(&code).unwrap(), format!("\"{expected}\""));
    }
}
