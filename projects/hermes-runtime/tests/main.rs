//! Hermes RPC 领域运行时的 mock 验证测试。
//!
//! 覆盖 loader / registry / dispatcher / wire envelope 的行为。artifact 合同
//! 本身的 serde 对齐与 span 校验由 `vos-core` 的 projection 测试负责；这里用
//! 自包含的最小 artifact 作为运行时输入。

use hermes_runtime::{
    AddHandler, BTreeServiceRegistry, EchoHandler, HermesDispatcher, HermesMethodDescriptor, HermesServiceContract,
    HermesServiceLoader, HermesServiceRegistry, JsonServiceLoader, error::ErrorKind, projection::HermesOperationKind,
};

/// 最小 service artifact：两个服务，均不带 methods。
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

/// 最小 operation artifact：query A / udf B / micro C。
const OPERATION_ARTIFACT: &str = r#"{
    "envelope": {
        "stage": "operation-syntax-projection",
        "contractFormatVersion": "vos-operation-contract-v0",
        "languageVersion": "vos-language-v0",
        "canonicalizationVersion": "vos-canonical-v0",
        "schemaFingerprint": null,
        "sourceUnits": [{
            "sourceUnitId": 0,
            "source": "query A(): U\nudf B(): V\nmicro C(): W"
        }]
    },
    "operations": [
        {
            "kind": "query",
            "name": "A",
            "identityKey": "query:A()->U",
            "operationId": "op_1234abcd5678ef90",
            "parameters": [],
            "returnType": {"named": ["U"]},
            "signature": null,
            "body": null,
            "span": {"start": 0, "end": 12}
        },
        {
            "kind": "udf",
            "name": "B",
            "identityKey": "udf:B()->V",
            "operationId": "op_abcdef0123456789",
            "parameters": [],
            "returnType": {"named": ["V"]},
            "signature": null,
            "body": null,
            "span": {"start": 13, "end": 23}
        },
        {
            "kind": "micro",
            "name": "C",
            "identityKey": "micro:C()->W",
            "operationId": "op_deadbeefcafebabe",
            "parameters": [],
            "returnType": {"named": ["W"]},
            "signature": null,
            "body": null,
            "span": {"start": 24, "end": 36}
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

// --- Loader 测试 --------------------------------------------------------

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
    assert_eq!(err.kind(), ErrorKind::Json);
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

// --- Registry 测试 ------------------------------------------------------

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
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("already registered"));
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

    // canonical path join 作 service_name
    let fs = registry.find("hermes.rpc.FileService").expect("FileService registered under canonical name");
    assert!(fs.methods.is_empty(), "projection stage 不带 methods");

    let notify = registry.find("hermes.rpc.Notify").expect("Notify registered under canonical name");
    assert!(notify.methods.is_empty());
}

#[test]
fn registry_rejects_duplicate_service_from_artifact() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_artifact(SERVICE_ARTIFACT.as_bytes()).expect("service projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_artifact(&artifact).unwrap();

    // 再注册一次 —— 所有 service_name 都已存在
    let err = registry.register_artifact(&artifact).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("already registered"));
}

#[test]
fn registry_register_operation_artifact_bulk_registers_all_operations() {
    let loader = JsonServiceLoader::default();
    let artifact = loader.load_operation_artifact(OPERATION_ARTIFACT.as_bytes()).expect("operation projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_operation_artifact(&artifact).expect("bulk register ok");

    assert_eq!(registry.all_operations().len(), 3);

    // 按 operation_id 查找
    let op_a = registry.find_operation("op_1234abcd5678ef90").expect("operation A found");
    assert_eq!(op_a.name, "A");
    assert_eq!(op_a.kind, HermesOperationKind::Query);

    // 按 name 查找
    let op_b = registry.find_operation_by_name("B").expect("operation B found by name");
    assert_eq!(op_b.operation_id, "op_abcdef0123456789");
    assert_eq!(op_b.kind, HermesOperationKind::Udf);

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
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("already registered"));
}

// --- 端到端：同时加载 service + operation artifact ----------------------

#[test]
fn registry_loads_service_and_operation_artifacts_together() {
    let loader = JsonServiceLoader::default();

    let svc = loader.load_artifact(SERVICE_ARTIFACT.as_bytes()).expect("service projection parses");
    let ops = loader.load_operation_artifact(OPERATION_ARTIFACT.as_bytes()).expect("operation projection parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_artifact(&svc).expect("services ok");
    registry.register_operation_artifact(&ops).expect("operations ok");

    // services 存在
    assert!(registry.find("hermes.rpc.FileService").is_some());
    assert!(registry.find("hermes.rpc.Notify").is_some());
    assert_eq!(registry.all().len(), 2);

    // operations 独立维护
    assert!(registry.find_operation("op_1234abcd5678ef90").is_some());
    assert!(registry.find_operation_by_name("A").is_some());
    assert_eq!(registry.all_operations().len(), 3);
}

// =======================================================================
// HermesDispatcher 端到端测试
// =======================================================================

/// 预加载两个 artifact 并在 A、B 上分别挂 EchoHandler / AddHandler。
fn dispatcher_with_handlers() -> HermesDispatcher {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_service_artifact(SERVICE_ARTIFACT).expect("service artifact ok");
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).expect("operation artifact ok");

    dispatcher.register_handler("op_1234abcd5678ef90", Box::new(EchoHandler)).expect("register EchoHandler for A");
    dispatcher.register_handler("op_abcdef0123456789", Box::new(AddHandler)).expect("register AddHandler for B");

    dispatcher
}

#[test]
fn dispatcher_dispatch_by_id_returns_handler_result() {
    let dispatcher = dispatcher_with_handlers();

    // EchoHandler: A 收到什么就回什么
    let params = serde_json::json!({"hello": "world", "n": 42});
    let got = dispatcher.dispatch("op_1234abcd5678ef90", &params).expect("dispatch A ok");
    assert_eq!(got, params);

    // AddHandler: B 算 a + b
    let got = dispatcher.dispatch("op_abcdef0123456789", &serde_json::json!({"a": 3, "b": 4})).expect("dispatch B ok");
    assert_eq!(got.as_i64(), Some(7));
}

#[test]
fn dispatcher_dispatch_by_name_routes_to_correct_operation() {
    let dispatcher = dispatcher_with_handlers();

    let got = dispatcher.dispatch_by_name("A", &serde_json::json!({"ping": true})).expect("dispatch_by_name A ok");
    assert_eq!(got, serde_json::json!({"ping": true}));

    let got = dispatcher.dispatch_by_name("B", &serde_json::json!({"a": 10, "b": 20})).expect("dispatch_by_name B ok");
    assert_eq!(got.as_i64(), Some(30));
}

#[test]
fn dispatcher_dispatch_without_handler_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    // fixture C（micro）没注册 handler
    let err = dispatcher.dispatch("op_deadbeefcafebabe", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("handler not registered"));
}

#[test]
fn dispatcher_dispatch_unknown_operation_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    let err = dispatcher.dispatch("op_0000000000000000", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("operation not found"));
}

#[test]
fn dispatcher_dispatch_by_name_unknown_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    let err = dispatcher.dispatch_by_name("NoSuchOperation", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("operation not found by name"));
}

#[test]
fn dispatch_request_response_roundtrip_and_success_path() {
    use hermes_runtime::{HermesDispatchRequest, HermesDispatchResponse};

    let dispatcher = dispatcher_with_handlers();

    // operation 是 operation_id → 直接路由
    let req = HermesDispatchRequest { operation: "op_1234abcd5678ef90".into(), params: serde_json::json!({"key": "val"}) };
    let resp = dispatcher.dispatch_request(&req);
    assert!(resp.result.is_some(), "success path should carry result");
    assert!(resp.error.is_none());
    assert_eq!(resp.operation_id, "op_1234abcd5678ef90");
    assert_eq!(resp.result.as_ref().unwrap(), &req.params);

    // roundtrip via JSON
    let resp_json = serde_json::to_string(&resp).expect("serialize response");
    let roundtrip: HermesDispatchResponse = serde_json::from_str(&resp_json).expect("deserialize response");
    assert_eq!(roundtrip, resp);

    let req_json = serde_json::to_string(&req).expect("serialize request");
    let roundtrip_req: HermesDispatchRequest = serde_json::from_str(&req_json).expect("deserialize request");
    assert_eq!(roundtrip_req, req);

    // operation 是 name → dispatch_by_name 路径
    let req_name = HermesDispatchRequest { operation: "B".into(), params: serde_json::json!({"a": 1, "b": 2}) };
    let resp_name = dispatcher.dispatch_request(&req_name);
    assert!(resp_name.result.is_some());
    assert_eq!(resp_name.operation_id, "op_abcdef0123456789");
    assert_eq!(resp_name.result.as_ref().unwrap().as_i64(), Some(3));

    // handler 未注册 → response 带 error 字段
    let req_no_handler = HermesDispatchRequest { operation: "op_deadbeefcafebabe".into(), params: serde_json::json!({}) };
    let resp_no_handler = dispatcher.dispatch_request(&req_no_handler);
    assert!(resp_no_handler.result.is_none());
    assert!(resp_no_handler.error.is_some());
    assert!(resp_no_handler.error.as_ref().unwrap().contains("handler not registered"));
    assert_eq!(resp_no_handler.operation_id, "op_deadbeefcafebabe");
}

// --- register_handler 防御逻辑 -------------------------------------------

#[test]
fn register_handler_rejects_malformed_operation_id() {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).unwrap();

    let err = dispatcher.register_handler("bad_id", Box::new(EchoHandler)).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("operation_id malformed"));
}

#[test]
fn register_handler_rejects_nonexistent_operation() {
    let mut dispatcher = HermesDispatcher::new();
    dispatcher.load_operation_artifact(OPERATION_ARTIFACT).unwrap();

    let err = dispatcher.register_handler("op_0000000000000000", Box::new(EchoHandler)).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("not found in registry"));
}
