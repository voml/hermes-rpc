//! Hermes RPC ?????? mock ?????
//!
//! ?? loader / registry / dispatcher / wire envelope ????artifact ??
//! ??? serde ??? span ????? `vos-types` ??????
//! ?????? artifact ????????

use std::{
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use hermes_runtime::{
    AddHandler, BTreeServiceRegistry, CallContext, CallOptions, CancelToken, EchoHandler, HermesDispatcher,
    HermesMethodDescriptor, HermesOperationHandler, HermesServiceContract, HermesServiceLoader, HermesServiceRegistry,
    JsonServiceLoader, RpcResult, WireCodec,
    codec::codec_for,
    error::RpcErrorCode,
    projection::{CanonicalType, OperationKind, OperationProjection},
};

/// ?? service artifact????????? methods?
const SERVICE_ARTIFACT: &str = r#"{
    "envelope": {
        "stage": "service-syntax-projection",
        "contractFormatVersion": "vos-service-contract-v0",
        "languageVersion": "vos-language-v0",
        "canonicalizationVersion": "vos-canonical-v0",
        "schemaFingerprint": null,
        "sourceUnits": [{
            "sourceUnitId": 0,
            "source": "service FileService {\n  read_file(path: utf8): FileContent\n}\n\nservice Notify {\n  send(to: utf8, msg: utf8): bool\n}\n"
        }]
    },
    "services": [
        {
            "name": "FileService",
            "canonicalPath": ["hermes", "rpc", "FileService"],
            "serviceId": "svc_fed285365413894d",
            "span": {"start": 0, "end": 55}
        },
        {
            "name": "Notify",
            "canonicalPath": ["hermes", "rpc", "Notify"],
            "serviceId": "svc_ce632aef610efc9d",
            "span": {"start": 57, "end": 100}
        }
    ]
}"#;

/// fixture operation artifact：无参数 echo A、带 `i64` 参数/返回的 B、无 handler 的 C。
const OPERATION_ARTIFACT: &str = r#"{
    "envelope": {
        "stage": "operation-syntax-projection",
        "contractFormatVersion": "vos-operation-contract-v0",
        "languageVersion": "vos-language-v0",
        "canonicalizationVersion": "vos-canonical-v0",
        "schemaFingerprint": null,
        "sourceUnits": [{
            "sourceUnitId": 0,
            "source": "query A()\nudf B(a: i64, b: i64): i64\nmicro C()"
        }]
    },
    "operations": [
        {
            "kind": "query",
            "name": "A",
            "identityKey": "query:A()",
            "operationId": "op_bcbc253a76d10ef5",
            "parameters": [],
            "returnType": null,
            "signature": null,
            "body": null,
            "span": {"start": 0, "end": 9}
        },
        {
            "kind": "udf",
            "name": "B",
            "identityKey": "udf:B(a:i64,b:i64)->i64",
            "operationId": "op_b6c3f1b0a7481391",
            "parameters": [
                {"name": "a", "typeExpr": {"named": ["i64"]}, "span": {"start": 16, "end": 17}},
                {"name": "b", "typeExpr": {"named": ["i64"]}, "span": {"start": 24, "end": 25}}
            ],
            "returnType": {"named": ["i64"]},
            "signature": null,
            "body": null,
            "span": {"start": 10, "end": 36}
        },
        {
            "kind": "micro",
            "name": "C",
            "identityKey": "micro:C()",
            "operationId": "op_fe2999f6bcf707d1",
            "parameters": [],
            "returnType": null,
            "signature": null,
            "body": null,
            "span": {"start": 37, "end": 46}
        }
    ]
}"#;

fn sample_contract_artifact() -> Vec<u8> {
    br#"{
        "service_name": "hermes.rpc.FileService",
        "revision": "vos-language-dev-a1b2c3",
        "methods": [
            {"name": "read_file",  "request_type": "ReadFileRequest",  "response_type": "FileContent"},
            {"name": "write_file", "request_type": "WriteFileRequest", "response_type": "WriteResult", "docs": "Persist file bytes"}
        ]
    }"#
    .to_vec()
}

// --- Loader ?? --------------------------------------------------------

#[test]
fn json_loader_parses_valid_artifact() {
    let loader = JsonServiceLoader::default();
    let contract = loader.load(&sample_contract_artifact()).expect("should parse");

    assert_eq!(contract.service_name, "hermes.rpc.FileService");
    assert_eq!(contract.revision.as_deref(), Some("vos-language-dev-a1b2c3"));
    assert_eq!(contract.methods.len(), 2);
    assert_eq!(contract.methods[0].name, "read_file");
    assert_eq!(contract.methods[1].docs, "Persist file bytes");
}

#[test]
fn json_loader_rejects_garbage() {
    let loader = JsonServiceLoader::default();
    let err = loader.load(b"not json").unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::InvalidRequest);
}

#[test]
fn json_loader_parses_service_projection_artifact() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_artifact(SERVICE_ARTIFACT.as_bytes()).expect("service projection parses");
    assert_eq!(artifact.services.len(), 2);
}

#[test]
fn json_loader_parses_operation_projection_artifact() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_operation_artifact(OPERATION_ARTIFACT.as_bytes()).expect("operation projection parses");
    assert_eq!(artifact.operations.len(), 3);
}

// --- Registry ?? ------------------------------------------------------

#[test]
fn registry_registers_and_finds() {
    let loader = JsonServiceLoader::default();
    let contract = loader.load(&sample_contract_artifact()).unwrap();

    let mut registry = BTreeServiceRegistry::default();
    registry.register(contract.clone()).expect("first register ok");

    let found = registry.find("hermes.rpc.FileService").expect("should find");
    assert_eq!(found, &contract);

    let method = registry.method("hermes.rpc.FileService", "write_file").expect("should find method");
    assert_eq!(method.response_type, "WriteResult");
}

#[test]
fn registry_rejects_duplicate_service_name() {
    let loader = JsonServiceLoader::default();
    let contract = loader.load(&sample_contract_artifact()).unwrap();

    let mut registry = BTreeServiceRegistry::default();
    registry.register(contract.clone()).unwrap();
    let err = registry.register(contract).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DuplicateRegistration);
    assert!(err.message().clone().contains("already registered"));
}

#[test]
fn registry_returns_none_for_unknown_service() {
    let registry = BTreeServiceRegistry::default();
    assert!(registry.find("no.such.Service").is_none());
    assert!(registry.method("no.such.Service", "x").is_none());
}

#[test]
fn registry_all_lists_registered() {
    let mut registry = BTreeServiceRegistry::default();

    let a = HermesServiceContract {
        service_name: "svc.A".into(),
        methods: vec![HermesMethodDescriptor {
            name: "ping".into(),
            request_type: "Ping".into(),
            response_type: "Pong".into(),
            docs: String::new(),
        }],
        revision: None,
    };
    let b = HermesServiceContract { service_name: "svc.B".into(), methods: vec![], revision: None };

    registry.register(a).unwrap();
    registry.register(b).unwrap();
    assert_eq!(registry.all().len(), 2);
}

#[test]
fn registry_register_artifact_bulk_registers_all_services() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_artifact(SERVICE_ARTIFACT.as_bytes()).expect("service projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_artifact(&artifact).expect("bulk register ok");

    assert_eq!(registry.all().len(), 2);

    // canonical path join ? service_name
    let fs = registry.find("hermes.rpc.FileService").expect("FileService registered under canonical name");
    assert!(fs.methods.is_empty(), "projection stage ?? methods");

    let notify = registry.find("hermes.rpc.Notify").expect("Notify registered under canonical name");
    assert!(notify.methods.is_empty());
}

#[test]
fn registry_rejects_duplicate_service_from_artifact() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_artifact(SERVICE_ARTIFACT.as_bytes()).expect("service projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_artifact(&artifact).unwrap();

    // ????? ?? ?? service_name ????
    let err = registry.register_artifact(&artifact).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DuplicateRegistration);
    assert!(err.message().clone().contains("already registered"));
}

#[test]
fn registry_register_operation_artifact_bulk_registers_all_operations() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_operation_artifact(OPERATION_ARTIFACT.as_bytes()).expect("operation projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_operation_artifact(&artifact).expect("bulk register ok");

    assert_eq!(registry.all_operations().len(), 3);

    // ? operation_id ??
    let op_a = registry.find_operation("op_bcbc253a76d10ef5").expect("operation A found");
    assert_eq!(op_a.name, "A");
    assert_eq!(op_a.kind, OperationKind::Query);

    // ? name ??
    let op_b = registry.find_operation_by_name("B").expect("operation B found by name");
    assert_eq!(op_b.operation_id, "op_b6c3f1b0a7481391");
    assert_eq!(op_b.kind, OperationKind::Udf);

    // unknown operation_id
    assert!(registry.find_operation("op_nonexistent000").is_none());
    assert!(registry.find_operation_by_name("NoSuchOp").is_none());
}

#[test]
fn registry_rejects_duplicate_operation() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_operation_artifact(OPERATION_ARTIFACT.as_bytes()).expect("operation projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_operation_artifact(&artifact).unwrap();

    let err = registry.register_operation_artifact(&artifact).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DuplicateRegistration);
    assert!(err.message().clone().contains("already registered"));
}

// --- ???????? service + operation artifact ----------------------

#[test]
fn registry_loads_service_and_operation_artifacts_together() {
    let loader = JsonServiceLoader::default();

    let svc = loader.load_artifact(SERVICE_ARTIFACT.as_bytes()).expect("service projection parses");
    let ops = loader.load_operation_artifact(OPERATION_ARTIFACT.as_bytes()).expect("operation projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_artifact(&svc).expect("services ok");
    registry.register_operation_artifact(&ops).expect("operations ok");

    // services ??
    assert!(registry.find("hermes.rpc.FileService").is_some());
    assert!(registry.find("hermes.rpc.Notify").is_some());
    assert_eq!(registry.all().len(), 2);

    // operations ????
    assert!(registry.find_operation("op_bcbc253a76d10ef5").is_some());
    assert!(registry.find_operation_by_name("A").is_some());
    assert_eq!(registry.all_operations().len(), 3);
}

// =======================================================================
// HermesDispatcher ?????
// =======================================================================

/// ????? artifact ?? A?B ???? EchoHandler / AddHandler?
fn dispatcher_with_handlers() -> HermesDispatcher {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_service_artifact(SERVICE_ARTIFACT).expect("service artifact ok");
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).expect("operation artifact ok");

    dispatcher.register_handler("op_bcbc253a76d10ef5", Box::new(EchoHandler)).expect("register EchoHandler for A");
    dispatcher.register_handler("op_b6c3f1b0a7481391", Box::new(AddHandler)).expect("register AddHandler for B");

    dispatcher
}

#[test]
fn dispatcher_dispatch_by_id_returns_handler_result() {
    let dispatcher = dispatcher_with_handlers();

    // EchoHandler: A 没有声明参数与返回类型，wire 参数原样回显。
    let params = serde_json::json!({});
    let got = dispatcher.dispatch("op_bcbc253a76d10ef5", &params).expect("dispatch A ok");
    assert_eq!(got, params);

    // AddHandler: B 的 `i64` 参数/返回按 codec 走 JSON string。
    let got = dispatcher.dispatch("op_b6c3f1b0a7481391", &serde_json::json!({"a": "3", "b": "4"})).expect("dispatch B ok");
    assert_eq!(got, serde_json::json!("7"));
}

#[test]
fn dispatcher_dispatch_by_name_routes_to_correct_operation() {
    let dispatcher = dispatcher_with_handlers();

    let got = dispatcher.dispatch_by_name("A", &serde_json::json!({})).expect("dispatch_by_name A ok");
    assert_eq!(got, serde_json::json!({}));

    let got = dispatcher.dispatch_by_name("B", &serde_json::json!({"a": "10", "b": "20"})).expect("dispatch_by_name B ok");
    assert_eq!(got, serde_json::json!("30"));
}

#[test]
fn dispatcher_dispatch_without_handler_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    // fixture C?micro???? handler
    let err = dispatcher.dispatch("op_fe2999f6bcf707d1", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::HandlerNotRegistered);
    assert!(err.message().clone().contains("handler not registered"));
}

#[test]
fn dispatcher_dispatch_unknown_operation_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    let err = dispatcher.dispatch("op_0000000000000000", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::UnknownOperation);
    assert!(err.message().clone().contains("operation not found"));
}

#[test]
fn dispatcher_dispatch_by_name_unknown_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    let err = dispatcher.dispatch_by_name("NoSuchOperation", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::UnknownOperation);
    assert!(err.message().clone().contains("operation not found by name"));
}

#[test]
fn dispatch_request_response_roundtrip_and_success_path() {
    use hermes_runtime::{HermesDispatchRequest, HermesDispatchResponse};

    let dispatcher = dispatcher_with_handlers();

    // operation 用 operation_id 时按 id 路由。
    let req = HermesDispatchRequest { operation: "op_bcbc253a76d10ef5".into(), params: serde_json::json!({}) };
    let resp = dispatcher.dispatch_request(&req);
    assert!(resp.result.is_some(), "success path should carry result");
    assert!(resp.error.is_none());
    assert_eq!(resp.operation_id, "op_bcbc253a76d10ef5");
    assert_eq!(resp.result.as_ref().unwrap(), &req.params);

    // roundtrip via JSON
    let resp_json = serde_json::to_string(&resp).expect("serialize response");
    let roundtrip: HermesDispatchResponse = serde_json::from_str(&resp_json).expect("deserialize response");
    assert_eq!(roundtrip, resp);

    let req_json = serde_json::to_string(&req).expect("serialize request");
    let roundtrip_req: HermesDispatchRequest = serde_json::from_str(&req_json).expect("deserialize request");
    assert_eq!(roundtrip_req, req);

    // operation ? name ? dispatch_by_name ??
    let req_name = HermesDispatchRequest { operation: "B".into(), params: serde_json::json!({"a": "1", "b": "2"}) };
    let resp_name = dispatcher.dispatch_request(&req_name);
    assert!(resp_name.result.is_some());
    assert_eq!(resp_name.operation_id, "op_b6c3f1b0a7481391");
    assert_eq!(resp_name.result.as_ref().unwrap(), &serde_json::json!("3"));

    // handler ??? ? response ???? error ??
    let req_no_handler = HermesDispatchRequest { operation: "op_fe2999f6bcf707d1".into(), params: serde_json::json!({}) };
    let resp_no_handler = dispatcher.dispatch_request(&req_no_handler);
    assert!(resp_no_handler.result.is_none());
    let wire_error = resp_no_handler.error.as_ref().expect("error should be present");
    assert_eq!(wire_error.code(), RpcErrorCode::HandlerNotRegistered);
    assert!(wire_error.message().contains("handler not registered"));
    assert_eq!(resp_no_handler.operation_id, "op_fe2999f6bcf707d1");

    // wire ??????`{"code": "...", "message": "..."}`
    let wire_json = serde_json::to_value(&resp_no_handler).expect("serialize response");
    assert_eq!(wire_json["error"]["code"], serde_json::json!("handler-not-registered"));
}

// --- register_handler ???? -------------------------------------------

#[test]
fn register_handler_rejects_malformed_operation_id() {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).unwrap();

    let err = dispatcher.register_handler("bad_id", Box::new(EchoHandler)).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::InvalidRequest);
    assert!(err.message().clone().contains("operation_id malformed"));
}

#[test]
fn register_handler_rejects_nonexistent_operation() {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).unwrap();

    let err = dispatcher.register_handler("op_0000000000000000", Box::new(EchoHandler)).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::UnknownOperation);
    assert!(err.message().clone().contains("not found in registry"));
}

// --- handler ?????? -------------------------------------------------

#[test]
fn add_handler_reports_invalid_request_for_missing_param() {
    let dispatcher = dispatcher_with_handlers();

    let err = dispatcher.dispatch("op_b6c3f1b0a7481391", &serde_json::json!({"a": "1"})).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::InvalidRequest);
    assert!(err.message().clone().contains("missing parameter `b`"));
}

#[test]
fn rpc_error_code_wire_names_are_kebab_case() {
    let codes = [
        (RpcErrorCode::ArtifactCorrupt, "artifact-corrupt"),
        (RpcErrorCode::UnknownOperation, "unknown-operation"),
        (RpcErrorCode::HandlerNotRegistered, "handler-not-registered"),
        (RpcErrorCode::DuplicateRegistration, "duplicate-registration"),
        (RpcErrorCode::InvalidRequest, "invalid-request"),
        (RpcErrorCode::UnsupportedCodec, "unsupported-codec"),
        (RpcErrorCode::HandlerFailed, "handler-failed"),
        (RpcErrorCode::DeadlineExceeded, "deadline-exceeded"),
        (RpcErrorCode::Cancelled, "cancelled"),
        (RpcErrorCode::Internal, "internal"),
    ];
    for (code, expected) in codes {
        assert_eq!(code.as_str(), expected);
        assert_eq!(serde_json::to_string(&code).unwrap(), format!("\"{expected}\""));
    }
}

// --- ?????deadline / cancellation / ???? -------------------------

const OP_A: &str = "op_bcbc253a76d10ef5";

fn dispatcher_with(handler: Box<dyn HermesOperationHandler>) -> HermesDispatcher {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).expect("operation artifact ok");
    dispatcher.register_handler(OP_A, handler).expect("register handler");
    dispatcher
}

/// ?? deadline ? handler???????????? handler ???
struct SlowHandler {
    delay: Duration,
}

impl HermesOperationHandler for SlowHandler {
    fn execute(
        &self,
        _ctx: &CallContext,
        _projection: &OperationProjection,
        _params: &serde_json::Value,
    ) -> RpcResult<serde_json::Value> {
        thread::sleep(self.delay);
        Ok(serde_json::json!({"slow": true}))
    }
}

/// ???????? handler?
struct CancelHandler;

impl HermesOperationHandler for CancelHandler {
    fn execute(
        &self,
        ctx: &CallContext,
        _projection: &OperationProjection,
        _params: &serde_json::Value,
    ) -> RpcResult<serde_json::Value> {
        if let Some(token) = ctx.token() {
            token.cancel();
        }
        Ok(serde_json::json!({"cancelled_self": true}))
    }
}

/// ??????????? gate ????
struct HoldHandler {
    started: mpsc::Sender<()>,
    gate: Arc<(Mutex<bool>, Condvar)>,
}

impl HermesOperationHandler for HoldHandler {
    fn execute(
        &self,
        _ctx: &CallContext,
        _projection: &OperationProjection,
        _params: &serde_json::Value,
    ) -> RpcResult<serde_json::Value> {
        let _ = self.started.send(());
        let (lock, condvar) = &*self.gate;
        let mut open = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        while !*open {
            open = condvar.wait(open).unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        Ok(serde_json::json!({"held": true}))
    }
}

fn release_gate(gate: &Arc<(Mutex<bool>, Condvar)>) {
    let (lock, condvar) = &**gate;
    *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    condvar.notify_all();
}

#[test]
fn dispatch_with_expired_deadline_returns_deadline_exceeded() {
    let dispatcher = dispatcher_with(Box::new(EchoHandler));
    let options = CallOptions::with_deadline(Instant::now() - Duration::from_millis(1));

    let err = dispatcher.dispatch_with(OP_A, &serde_json::json!({}), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DeadlineExceeded);
}

#[test]
fn dispatch_within_deadline_returns_result() {
    let dispatcher = dispatcher_with(Box::new(EchoHandler));
    let options = CallOptions::with_timeout(Duration::from_secs(30));

    let got = dispatcher.dispatch_with(OP_A, &serde_json::json!({}), &options).unwrap();
    assert_eq!(got, serde_json::json!({}));
}

#[test]
fn dispatch_times_out_when_handler_outlives_deadline() {
    let dispatcher = dispatcher_with(Box::new(SlowHandler { delay: Duration::from_millis(80) }));
    let options = CallOptions::with_timeout(Duration::from_millis(10));

    let err = dispatcher.dispatch_with(OP_A, &serde_json::json!({}), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DeadlineExceeded);
}

#[test]
fn dispatch_with_cancelled_token_returns_cancelled() {
    let dispatcher = dispatcher_with(Box::new(EchoHandler));
    let token = CancelToken::new();
    token.cancel();
    let options = CallOptions::new().and_cancellation(token);

    let err = dispatcher.dispatch_with(OP_A, &serde_json::json!({}), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::Cancelled);
}

#[test]
fn handler_can_cancel_its_own_call() {
    let dispatcher = dispatcher_with(Box::new(CancelHandler));
    let options = CallOptions::new().and_cancellation(CancelToken::new());

    let err = dispatcher.dispatch_with(OP_A, &serde_json::json!({}), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::Cancelled);
}

#[test]
fn concurrency_limit_queues_until_permit_released() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (started, started_rx) = mpsc::channel();

    let mut dispatcher = dispatcher_with(Box::new(HoldHandler { started, gate: gate.clone() }));
    dispatcher.set_max_concurrency(Some(1));
    assert_eq!(dispatcher.max_concurrency(), Some(1));

    let dispatcher = Arc::new(dispatcher);
    let background = {
        let dispatcher = Arc::clone(&dispatcher);
        thread::spawn(move || dispatcher.dispatch(OP_A, &serde_json::json!({})))
    };
    started_rx.recv_timeout(Duration::from_secs(5)).expect("handler should have started");

    // ??????????????? deadline??? deadline-exceeded
    let options = CallOptions::with_timeout(Duration::from_millis(30));
    let err = dispatcher.dispatch_with(OP_A, &serde_json::json!({}), &options).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::DeadlineExceeded);

    release_gate(&gate);
    let held = background.join().expect("background thread joins").expect("held dispatch succeeds");
    assert_eq!(held, serde_json::json!({"held": true}));

    // 许可归还后同一参数再次通过。
    let options = CallOptions::with_timeout(Duration::from_millis(500));
    let got = dispatcher.dispatch_with(OP_A, &serde_json::json!({}), &options).expect("permit released");
    assert_eq!(got, serde_json::json!({"held": true}));
}

// =======================================================================
// codec：canonical type → wire 表达
// =======================================================================

/// fixture operation artifact：列表 / 可选参数与未知 Named / Generic。
const CODEC_ARTIFACT: &str = r#"{
    "envelope": {
        "stage": "operation-syntax-projection",
        "contractFormatVersion": "vos-operation-contract-v0",
        "languageVersion": "vos-language-v0",
        "canonicalizationVersion": "vos-canonical-v0",
        "schemaFingerprint": null,
        "sourceUnits": [{
            "sourceUnitId": 0,
            "source": "query Join(items: List<i64>, suffix: Optional<utf8>): List<utf8>\nquery RejectNamed(x: Missing): utf8\nquery RejectGeneric(x: Box<utf8>): utf8"
        }]
    },
    "operations": [
        {
            "kind": "query",
            "name": "Join",
            "identityKey": "query:Join(items:List<i64>,suffix:Optional<utf8>)->List<utf8>",
            "operationId": "op_f5a61512f163f99c",
            "parameters": [
                {"name": "items", "typeExpr": {"list": {"named": ["i64"]}}, "span": {"start": 11, "end": 16}},
                {"name": "suffix", "typeExpr": {"optional": {"named": ["utf8"]}}, "span": {"start": 29, "end": 35}}
            ],
            "returnType": {"list": {"named": ["utf8"]}},
            "signature": null,
            "body": null,
            "span": {"start": 0, "end": 64}
        },
        {
            "kind": "query",
            "name": "RejectNamed",
            "identityKey": "query:RejectNamed(x:Missing)->utf8",
            "operationId": "op_ecdcd0a1a4f7ef39",
            "parameters": [
                {"name": "x", "typeExpr": {"named": ["Missing"]}, "span": {"start": 82, "end": 83}}
            ],
            "returnType": {"named": ["utf8"]},
            "signature": null,
            "body": null,
            "span": {"start": 65, "end": 100}
        },
        {
            "kind": "query",
            "name": "RejectGeneric",
            "identityKey": "query:RejectGeneric(x:Box<utf8>)->utf8",
            "operationId": "op_319f00a433994c58",
            "parameters": [
                {
                    "name": "x",
                    "typeExpr": {"generic": {"path": ["Box"], "arguments": [{"type": {"named": ["utf8"]}}]}},
                    "span": {"start": 120, "end": 121}
                }
            ],
            "returnType": {"named": ["utf8"]},
            "signature": null,
            "body": null,
            "span": {"start": 101, "end": 140}
        }
    ]
}"#;

/// 把 handler 看到的 `items`（`i64` wire 字符串）拼上 `suffix` 后返回。
struct JoinHandler;

impl HermesOperationHandler for JoinHandler {
    fn execute(
        &self,
        _ctx: &CallContext,
        _projection: &OperationProjection,
        params: &serde_json::Value,
    ) -> RpcResult<serde_json::Value> {
        let items = params.get("items").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        let suffix = params.get("suffix").and_then(serde_json::Value::as_str).unwrap_or_default();
        let joined = items
            .into_iter()
            .map(|item| serde_json::Value::String(format!("{}{suffix}", item.as_str().unwrap_or_default())))
            .collect();
        Ok(serde_json::Value::Array(joined))
    }
}

/// 返回 JSON number 的 handler，用于验证返回类型 codec 校验。
struct NumericReturnHandler;

impl HermesOperationHandler for NumericReturnHandler {
    fn execute(
        &self,
        _ctx: &CallContext,
        _projection: &OperationProjection,
        _params: &serde_json::Value,
    ) -> RpcResult<serde_json::Value> {
        Ok(serde_json::json!(7))
    }
}

fn codec_dispatcher() -> HermesDispatcher {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_operation_artifact(CODEC_ARTIFACT).expect("codec artifact ok");
    dispatcher.register_handler("op_f5a61512f163f99c", Box::new(JoinHandler)).expect("register JoinHandler");
    dispatcher.register_handler("op_ecdcd0a1a4f7ef39", Box::new(EchoHandler)).expect("register RejectNamed handler");
    dispatcher.register_handler("op_319f00a433994c58", Box::new(EchoHandler)).expect("register RejectGeneric handler");
    dispatcher
}

#[test]
fn codec_decodes_list_and_optional_parameters() {
    let dispatcher = codec_dispatcher();

    let got = dispatcher
        .dispatch_by_name("Join", &serde_json::json!({"items": ["1", "2", "3"], "suffix": "!"}))
        .expect("list + optional dispatch ok");
    assert_eq!(got, serde_json::json!(["1!", "2!", "3!"]));

    // 可选参数为 null 时 handler 看到 null。
    let got = dispatcher
        .dispatch_by_name("Join", &serde_json::json!({"items": ["1"], "suffix": null}))
        .expect("optional null dispatch ok");
    assert_eq!(got, serde_json::json!(["1"]));
}

#[test]
fn codec_i64_wire_string_reaches_handler_unchanged() {
    let dispatcher = codec_dispatcher();

    // handler 侧的 JoinHandler 以字符串拼接，证明 `i64` 在 handler 里仍是 wire string。
    let got = dispatcher
        .dispatch_by_name("Join", &serde_json::json!({"items": ["-9223372036854775808"], "suffix": ""}))
        .expect("i64 boundary dispatch ok");
    assert_eq!(got, serde_json::json!(["-9223372036854775808"]));
}

#[test]
fn codec_rejects_missing_extra_and_mistyped_parameters() {
    let dispatcher = dispatcher_with_handlers();

    let missing = dispatcher.dispatch_by_name("B", &serde_json::json!({"a": "1"})).unwrap_err();
    assert_eq!(missing.code(), RpcErrorCode::InvalidRequest);
    assert!(missing.message().contains("missing parameter `b`"));

    let extra = dispatcher.dispatch_by_name("A", &serde_json::json!({"x": 1})).unwrap_err();
    assert_eq!(extra.code(), RpcErrorCode::InvalidRequest);
    assert!(extra.message().contains("unknown parameter `x`"));

    let mistyped = dispatcher.dispatch_by_name("B", &serde_json::json!({"a": 1, "b": "2"})).unwrap_err();
    assert_eq!(mistyped.code(), RpcErrorCode::InvalidRequest);
    assert!(mistyped.message().contains("parameter `a` must be JSON string"));

    let not_object = dispatcher.dispatch_by_name("A", &serde_json::json!([])).unwrap_err();
    assert_eq!(not_object.code(), RpcErrorCode::InvalidRequest);
    assert!(not_object.message().contains("params must be a JSON object"));
}

#[test]
fn codec_rejects_unknown_named_and_generic_types() {
    let dispatcher = codec_dispatcher();

    let named = dispatcher.dispatch_by_name("RejectNamed", &serde_json::json!({"x": "1"})).unwrap_err();
    assert_eq!(named.code(), RpcErrorCode::UnsupportedCodec);
    assert!(named.message().contains("unsupported codec for type `Missing`"));

    let generic = dispatcher.dispatch_by_name("RejectGeneric", &serde_json::json!({"x": "1"})).unwrap_err();
    assert_eq!(generic.code(), RpcErrorCode::UnsupportedCodec);
    assert!(generic.message().contains("unsupported codec for type `Box<utf8>`"));
}

#[test]
fn codec_unsupported_error_shape_matches_wire_body() {
    let dispatcher = codec_dispatcher();

    let request =
        hermes_runtime::HermesDispatchRequest { operation: "RejectNamed".into(), params: serde_json::json!({"x": "1"}) };
    let response = dispatcher.dispatch_request(&request);

    let wire = serde_json::to_value(&response).expect("serialize response");
    assert_eq!(wire["error"]["code"], serde_json::json!("unsupported-codec"));
    assert!(wire["error"]["message"].as_str().is_some_and(|message| message.contains("Missing")));
    assert_eq!(wire["operationId"], serde_json::json!("op_ecdcd0a1a4f7ef39"));
    assert!(wire.get("result").is_none());
}

#[test]
fn codec_rejects_mistyped_return_value() {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).expect("operation artifact ok");
    dispatcher.register_handler("op_b6c3f1b0a7481391", Box::new(NumericReturnHandler)).expect("register numeric handler");

    let err = dispatcher.dispatch_by_name("B", &serde_json::json!({"a": "1", "b": "2"})).unwrap_err();
    assert_eq!(err.code(), RpcErrorCode::HandlerFailed);
    assert!(err.message().contains("return value must be JSON string"));
}

#[test]
fn codec_table_covers_scalars_and_rejects_any() {
    let named = |name: &str| CanonicalType::Named(vec![name.to_string()]);

    assert_eq!(codec_for(&named("bool")).unwrap(), WireCodec::Bool);
    assert_eq!(codec_for(&named("utf8")).unwrap(), WireCodec::Utf8);
    assert_eq!(codec_for(&named("i32")).unwrap(), WireCodec::Integer { min: i64::from(i32::MIN), max: i64::from(i32::MAX) });
    assert_eq!(codec_for(&named("f64")).unwrap(), WireCodec::Float);
    assert_eq!(codec_for(&named("i64")).unwrap(), WireCodec::Int64);
    assert_eq!(codec_for(&named("u64")).unwrap(), WireCodec::Uint64);
    assert_eq!(codec_for(&named("int")).unwrap(), WireCodec::IntegerString);
    assert_eq!(codec_for(&named("decimal")).unwrap(), WireCodec::Decimal);
    assert_eq!(codec_for(&named("uuid")).unwrap(), WireCodec::Uuid);
    assert_eq!(codec_for(&named("date")).unwrap(), WireCodec::Date);
    assert_eq!(codec_for(&named("time")).unwrap(), WireCodec::Time);
    assert_eq!(codec_for(&named("datetime")).unwrap(), WireCodec::DateTime);
    assert_eq!(codec_for(&named("datetimeUtc")).unwrap(), WireCodec::DateTimeUtc);
    assert_eq!(codec_for(&named("bytes")).unwrap(), WireCodec::Bytes);
    assert_eq!(
        codec_for(&CanonicalType::Optional(Box::new(named("utf8")))).unwrap(),
        WireCodec::Optional(Box::new(WireCodec::Utf8))
    );
    assert_eq!(
        codec_for(&CanonicalType::List(Box::new(named("i64")))).unwrap(),
        WireCodec::List(Box::new(WireCodec::Int64))
    );
    assert_eq!(
        codec_for(&CanonicalType::Reference(Box::new(named("uuid")))).unwrap(),
        WireCodec::Uuid
    );

    for unsupported in ["json", "value", "any", "Missing"] {
        let err = codec_for(&named(unsupported)).unwrap_err();
        assert_eq!(err.code(), RpcErrorCode::UnsupportedCodec);
    }
}
