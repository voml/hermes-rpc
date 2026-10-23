//! Hermes 接口骨架的 mock 验证测试。
//!
//! 这些测试不做真实 RPC 运行时，只验证 `HermesServiceLoader` /
//! `HermesServiceRegistry` trait 能被 mock 实现、artifact 能被 JSON loader
//! 解析、registry 能正确注册和查找服务。
//!
//! 同时覆盖 Hermes 对 `vos-language` `ServiceProjectionArtifact` 的本地镜像
//! 解析：`HermesServiceArtifact` 与上游 JSON 形状必须完全兼容。

use vos_core::hermes::{
    BTreeServiceRegistry, HermesMethodDescriptor, HermesOperationArtifact,
    HermesOperationKind, HermesServiceArtifact, HermesServiceContract,
    HermesServiceLoader, HermesServiceRegistry, JsonServiceLoader,
};
use vos_core::error::{ErrorKind, QError};

fn sample_artifact() -> Vec<u8> {
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
    let contract = loader.load(&sample_artifact()).expect("should parse");

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

// --- Registry 测试 ------------------------------------------------------

#[test]
fn registry_registers_and_finds() {
    let loader = JsonServiceLoader::default();
    let contract = loader.load(&sample_artifact()).unwrap();

    let mut registry = BTreeServiceRegistry::default();
    registry.register(contract.clone()).expect("first register ok");

    let found = registry.find("hermes.rpc.FileService").expect("should find");
    assert_eq!(found, &contract);

    let method = registry
        .method("hermes.rpc.FileService", "write_file")
        .expect("should find method");
    assert_eq!(method.response_type, "WriteResult");
}

#[test]
fn registry_rejects_duplicate_service_name() {
    let loader = JsonServiceLoader::default();
    let contract = loader.load(&sample_artifact()).unwrap();

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
    let b = HermesServiceContract {
        service_name: "svc.B".into(),
        methods: vec![],
        revision: None,
    };

    registry.register(a).unwrap();
    registry.register(b).unwrap();
    assert_eq!(registry.all().len(), 2);
}

// --- QError 构造器烟雾测试 -----------------------------------------------

#[test]
fn qerror_variants_build_expected_kinds() {
    let e = QError::syntax_error("bad");
    assert_eq!(e.kind(), ErrorKind::Syntax);

    let e = QError::validation_error("nope");
    assert_eq!(e.kind(), ErrorKind::Validation);
}

// --- ServiceProjectionArtifact fixture 测试 -----------------------------

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/service_projection.basic.json")
}

#[test]
fn load_artifact_from_file_parses_vos_projection_fixture() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_artifact_file(&fixture_path())
        .expect("should parse projection fixture");

    assert_eq!(artifact.envelope.stage, "service-syntax-projection");
    assert_eq!(artifact.envelope.contract_format_version, "vos-service-contract-v0");
    assert_eq!(artifact.envelope.language_version, "vos-language-v0");
    assert_eq!(artifact.envelope.source_units.len(), 1);
    assert_eq!(
        artifact.envelope.source_units[0].source_unit_id,
        0
    );
    assert_eq!(artifact.services.len(), 2);
}

#[test]
fn load_artifact_from_bytes_parses_vos_projection() {
    let loader = JsonServiceLoader::default();
    let bytes = std::fs::read(fixture_path()).expect("fixture exists");
    let artifact = loader
        .load_artifact(&bytes)
        .expect("should parse projection from bytes");

    // 校验第一个服务
    let first = &artifact.services[0];
    assert_eq!(first.name, "FileService");
    assert_eq!(first.canonical_path, vec!["hermes", "rpc", "FileService"]);
    assert_eq!(first.span, (0usize..55usize).into());
    // 校验第二个服务
    let second = &artifact.services[1];
    assert_eq!(second.name, "Notify");
    assert_eq!(second.canonical_path, vec!["hermes", "rpc", "Notify"]);
    assert_eq!(second.span, (57usize..100usize).into());
}

#[test]
fn registry_register_artifact_bulk_registers_all_services() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_artifact_file(&fixture_path())
        .expect("fixture parses");

    let mut registry = BTreeServiceRegistry::default();
    registry
        .register_artifact(&artifact)
        .expect("bulk register ok");

    assert_eq!(registry.all().len(), 2);

    // canonical path join 作 service_name
    let fs = registry
        .find("hermes.rpc.FileService")
        .expect("FileService registered under canonical name");
    assert!(fs.methods.is_empty(), "projection stage 不带 methods");

    let notify = registry
        .find("hermes.rpc.Notify")
        .expect("Notify registered under canonical name");
    assert!(notify.methods.is_empty());
}

#[test]
fn artifact_version_mismatch_is_rejected() {
    let loader = JsonServiceLoader::default();

    // contractFormatVersion 不对
    let bad = br#"{
        "envelope": {
            "stage": "service-syntax-projection",
            "contractFormatVersion": "wrong-v99",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{"sourceUnitId": 0, "source": "service A {}"}]
        },
        "services": [{"name": "A", "canonicalPath": ["A"], "span": {"start": 0, "end": 10}}]
    }"#;

    let err = loader.load_artifact(bad).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("unsupported service artifact version"));
}

#[test]
fn artifact_rejects_unknown_fields() {
    let loader = JsonServiceLoader::default();
    let with_extra = br#"{
        "envelope": {
            "stage": "service-syntax-projection",
            "contractFormatVersion": "vos-service-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{"sourceUnitId": 0, "source": "service A {}"}],
            "extraFieldShouldNotExist": 42
        },
        "services": [{"name": "A", "canonicalPath": ["A"], "span": {"start": 0, "end": 10}}]
    }"#;

    let err = loader.load_artifact(with_extra).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Json, "deny_unknown_fields 应将其视为 JSON 错误");
}

#[test]
fn artifact_roundtrip_via_to_json_from_json() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_artifact_file(&fixture_path())
        .expect("fixture parses");

    let json = artifact.to_json().expect("serialize");
    let roundtrip = HermesServiceArtifact::from_json(&json).expect("roundtrip");

    assert_eq!(roundtrip, artifact);
}

#[test]
fn registry_rejects_duplicate_service_from_artifact() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_artifact_file(&fixture_path())
        .expect("fixture parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_artifact(&artifact).unwrap();

    // 再注册一次 —— 所有 service_name 都已存在
    let err = registry.register_artifact(&artifact).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("already registered"));
}

// --- span 校验测试 ------------------------------------------------------

#[test]
fn artifact_rejects_service_span_out_of_bounds() {
    // source 只有 5 个字节 "hello"，但 span end=999
    let bad = br#"{
        "envelope": {
            "stage": "service-syntax-projection",
            "contractFormatVersion": "vos-service-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{"sourceUnitId": 0, "source": "hello"}]
        },
        "services": [
            {"name": "A", "canonicalPath": ["A"], "span": {"start": 0, "end": 999}}
        ]
    }"#;

    let err = HermesServiceArtifact::from_json(
        std::str::from_utf8(bad).unwrap(),
    )
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(
        err.message().contains("service span out of bounds"),
        "got: {}",
        err.message()
    );
}

#[test]
fn artifact_rejects_service_span_end_before_start_via_deserialize() {
    // span start > end 在反序列化层就会拒绝
    let bad = br#"{
        "envelope": {
            "stage": "service-syntax-projection",
            "contractFormatVersion": "vos-service-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{"sourceUnitId": 0, "source": "hello"}]
        },
        "services": [
            {"name": "A", "canonicalPath": ["A"], "span": {"start": 10, "end": 2}}
        ]
    }"#;

    let err = HermesServiceArtifact::from_json(
        std::str::from_utf8(bad).unwrap(),
    )
    .unwrap_err();
    // deserialize_span 直接返回 serde 错误
    assert!(
        err.kind() == ErrorKind::Json || err.kind() == ErrorKind::Validation,
        "unexpected error kind: {:?}",
        err.kind()
    );
}

#[test]
fn artifact_rejects_body_span_out_of_bounds() {
    // source 只有 "service A {}" (12 字节)，body span 越界
    let bad = br#"{
        "envelope": {
            "stage": "service-syntax-projection",
            "contractFormatVersion": "vos-service-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{"sourceUnitId": 0, "source": "service A {}"}]
        },
        "services": [
            {"name": "A", "canonicalPath": ["A"], "span": {"start": 0, "end": 12},
             "body": {"text": "body", "span": {"start": 20, "end": 50}}}
        ]
    }"#;

    let err = HermesServiceArtifact::from_json(
        std::str::from_utf8(bad).unwrap(),
    )
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(
        err.message().contains("service body span out of bounds"),
        "got: {}",
        err.message()
    );
}

#[test]
fn artifact_accepts_valid_char_boundary_spans() {
    // 构造一个带多字节字符的 source，验证 span 必须落在 char boundary
    // 使用普通字符串（不是 raw byte string）来避免编译警告
    let source = "héllo wörld";
    let json = format!(
        r#"{{
        "envelope": {{
            "stage": "service-syntax-projection",
            "contractFormatVersion": "vos-service-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{{"sourceUnitId": 0, "source": "{source}"}}]
        }},
        "services": [
            {{"name": "A", "canonicalPath": ["A"], "span": {{"start": 0, "end": {len}}}}}
        ]
    }}"#,
        source = source.replace('"', "\\\""),
        len = source.len(),
    );

    let artifact = HermesServiceArtifact::from_json(&json)
        .expect("char-boundary span should be accepted");
    assert_eq!(artifact.services.len(), 1);
}

// --- HermesOperationArtifact fixture 测试 --------------------------------

fn operation_fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/operation_projection.basic.json")
}

#[test]
fn load_operation_artifact_from_file_parses_fixture() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_operation_artifact_file(&operation_fixture_path())
        .expect("should parse operation fixture");

    assert_eq!(
        artifact.envelope.stage,
        "operation-syntax-projection"
    );
    assert_eq!(
        artifact.envelope.contract_format_version,
        "vos-operation-contract-v0"
    );
    assert_eq!(artifact.operations.len(), 3);
}

#[test]
fn load_operation_artifact_parses_kind_name_and_span() {
    let loader = JsonServiceLoader::default();
    let bytes = std::fs::read(operation_fixture_path()).expect("fixture exists");
    let artifact = loader
        .load_operation_artifact(&bytes)
        .expect("should parse from bytes");

    let first = &artifact.operations[0];
    assert_eq!(first.kind, HermesOperationKind::Query);
    assert_eq!(first.name, "A");
    assert_eq!(first.span, (0usize..12usize).into());
    assert_eq!(first.operation_id, "op_1234abcd5678ef90");

    let second = &artifact.operations[1];
    assert_eq!(second.kind, HermesOperationKind::Udf);
    assert_eq!(second.name, "B");
    assert_eq!(second.span, (13usize..23usize).into());

    let third = &artifact.operations[2];
    assert_eq!(third.kind, HermesOperationKind::Micro);
    assert_eq!(third.name, "C");
    assert_eq!(third.span, (24usize..36usize).into());
}

#[test]
fn operation_artifact_roundtrip_via_to_json_from_json() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_operation_artifact_file(&operation_fixture_path())
        .expect("fixture parses");

    let json = artifact.to_json().expect("serialize");
    let roundtrip = HermesOperationArtifact::from_json(&json).expect("roundtrip");

    assert_eq!(roundtrip, artifact);
}

#[test]
fn operation_artifact_rejects_unknown_stage() {
    let bad = br#"{
        "envelope": {
            "stage": "wrong-stage",
            "contractFormatVersion": "vos-operation-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{"sourceUnitId": 0, "source": "query X():"}]
        },
        "operations": []
    }"#;

    let err = HermesOperationArtifact::from_json(
        std::str::from_utf8(bad).unwrap(),
    )
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
}

#[test]
fn operation_artifact_rejects_malformed_operation_id() {
    let bad = br#"{
        "envelope": {
            "stage": "operation-syntax-projection",
            "contractFormatVersion": "vos-operation-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{"sourceUnitId": 0, "source": "query X(): Y"}]
        },
        "operations": [
            {"kind": "query", "name": "X", "identityKey": "query:X()->Y",
             "operationId": "not_a_valid_id",
             "parameters": [], "returnType": null,
             "signature": null, "body": null,
             "span": {"start": 0, "end": 12}}
        ]
    }"#;

    let err = HermesOperationArtifact::from_json(
        std::str::from_utf8(bad).unwrap(),
    )
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("operation id is malformed"));
}

// --- Registry + OperationArtifact 组合测试 ------------------------------

#[test]
fn registry_register_operation_artifact_bulk_registers_all_operations() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_operation_artifact_file(&operation_fixture_path())
        .expect("fixture parses");

    let mut registry = BTreeServiceRegistry::default();
    registry
        .register_operation_artifact(&artifact)
        .expect("bulk register ok");

    assert_eq!(registry.all_operations().len(), 3);

    // 按 operation_id 查找
    let op_a = registry
        .find_operation("op_1234abcd5678ef90")
        .expect("operation A found");
    assert_eq!(op_a.name, "A");
    assert_eq!(op_a.kind, HermesOperationKind::Query);

    // 按 name 查找
    let op_b = registry
        .find_operation_by_name("B")
        .expect("operation B found by name");
    assert_eq!(op_b.operation_id, "op_abcdef0123456789");
    assert_eq!(op_b.kind, HermesOperationKind::Udf);

    // unknown operation_id
    assert!(registry.find_operation("op_nonexistent000").is_none());
    assert!(registry.find_operation_by_name("NoSuchOp").is_none());
}

#[test]
fn registry_rejects_duplicate_operation() {
    let loader = JsonServiceLoader::default();
    let artifact = loader
        .load_operation_artifact_file(&operation_fixture_path())
        .expect("fixture parses");

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

    let svc = loader
        .load_artifact_file(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/service_projection.basic.json"),
        )
        .expect("service fixture parses");

    let ops = loader
        .load_operation_artifact_file(&operation_fixture_path())
        .expect("operation fixture parses");

    let mut registry = BTreeServiceRegistry::default();
    registry.register_artifact(&svc).expect("services ok");
    registry
        .register_operation_artifact(&ops)
        .expect("operations ok");

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

fn service_fixture_json() -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/service_projection.basic.json"),
    )
    .expect("service fixture exists")
}

fn operation_fixture_json() -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/operation_projection.basic.json"),
    )
    .expect("operation fixture exists")
}

/// 预加载两个 fixture 并在 A、B 上分别挂 EchoHandler / AddHandler。
fn dispatcher_with_handlers() -> vos_core::hermes::HermesDispatcher {
    use vos_core::hermes::{AddHandler, EchoHandler, HermesDispatcher};

    let mut dispatcher = HermesDispatcher::new();
    dispatcher
        .load_service_artifact(&service_fixture_json())
        .expect("service fixture ok");
    dispatcher
        .load_operation_artifact(&operation_fixture_json())
        .expect("operation fixture ok");

    dispatcher
        .register_handler(
            "op_1234abcd5678ef90",
            Box::new(EchoHandler),
        )
        .expect("register EchoHandler for A");
    dispatcher
        .register_handler(
            "op_abcdef0123456789",
            Box::new(AddHandler),
        )
        .expect("register AddHandler for B");

    dispatcher
}

// --- 测试 1：按 operation_id dispatch 成功 --------------------------------

#[test]
fn dispatcher_dispatch_by_id_returns_handler_result() {
    let dispatcher = dispatcher_with_handlers();

    // EchoHandler: A 收到什么就回什么
    let params = serde_json::json!({"hello": "world", "n": 42});
    let got = dispatcher
        .dispatch("op_1234abcd5678ef90", &params)
        .expect("dispatch A ok");
    assert_eq!(got, params);

    // AddHandler: B 算 a + b
    let got = dispatcher
        .dispatch("op_abcdef0123456789", &serde_json::json!({"a": 3, "b": 4}))
        .expect("dispatch B ok");
    assert_eq!(got.as_i64(), Some(7));
}

// --- 测试 2：dispatch_by_name --------------------------------------------

#[test]
fn dispatcher_dispatch_by_name_routes_to_correct_operation() {
    let dispatcher = dispatcher_with_handlers();

    let got = dispatcher
        .dispatch_by_name("A", &serde_json::json!({"ping": true}))
        .expect("dispatch_by_name A ok");
    assert_eq!(got, serde_json::json!({"ping": true}));

    let got = dispatcher
        .dispatch_by_name("B", &serde_json::json!({"a": 10, "b": 20}))
        .expect("dispatch_by_name B ok");
    assert_eq!(got.as_i64(), Some(30));
}

// --- 测试 3：handler not registered --------------------------------------

#[test]
fn dispatcher_dispatch_without_handler_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    // fixture C（micro）没注册 handler
    let err = dispatcher
        .dispatch("op_deadbeefcafebabe", &serde_json::json!({}))
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("handler not registered"));
}

// --- 测试 4：operation not found -----------------------------------------

#[test]
fn dispatcher_dispatch_unknown_operation_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    let err = dispatcher
        .dispatch("op_0000000000000000", &serde_json::json!({}))
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("operation not found"));
}

#[test]
fn dispatcher_dispatch_by_name_unknown_returns_error() {
    let dispatcher = dispatcher_with_handlers();

    let err = dispatcher
        .dispatch_by_name("NoSuchOperation", &serde_json::json!({}))
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Custom);
    assert!(err.message().contains("operation not found by name"));
}

// --- 测试 5：HermesDispatchRequest / Response JSON roundtrip -------------

#[test]
fn dispatch_request_response_roundtrip_and_success_path() {
    use vos_core::hermes::{HermesDispatchRequest, HermesDispatchResponse};

    let dispatcher = dispatcher_with_handlers();

    // 5a：operation 是 operation_id → 直接路由
    let req = HermesDispatchRequest {
        operation: "op_1234abcd5678ef90".into(),
        params: serde_json::json!({"key": "val"}),
    };
    let resp = dispatcher.dispatch_request(&req);
    assert!(resp.result.is_some(), "success path should carry result");
    assert!(resp.error.is_none());
    assert_eq!(resp.operation_id, "op_1234abcd5678ef90");
    assert_eq!(resp.result.as_ref().unwrap(), &req.params);

    // roundtrip via JSON
    let resp_json = serde_json::to_string(&resp).expect("serialize response");
    let roundtrip: HermesDispatchResponse =
        serde_json::from_str(&resp_json).expect("deserialize response");
    assert_eq!(roundtrip, resp);

    let req_json = serde_json::to_string(&req).expect("serialize request");
    let roundtrip_req: HermesDispatchRequest =
        serde_json::from_str(&req_json).expect("deserialize request");
    assert_eq!(roundtrip_req, req);

    // 5b：operation 是 name → dispatch_by_name 路径
    let req_name = HermesDispatchRequest {
        operation: "B".into(),
        params: serde_json::json!({"a": 1, "b": 2}),
    };
    let resp_name = dispatcher.dispatch_request(&req_name);
    assert!(resp_name.result.is_some());
    assert_eq!(resp_name.operation_id, "op_abcdef0123456789");
    assert_eq!(resp_name.result.as_ref().unwrap().as_i64(), Some(3));

    // 5c：handler 未注册 → response 带 error 字段
    let req_no_handler = HermesDispatchRequest {
        operation: "op_deadbeefcafebabe".into(),
        params: serde_json::json!({}),
    };
    let resp_no_handler = dispatcher.dispatch_request(&req_no_handler);
    assert!(resp_no_handler.result.is_none());
    assert!(resp_no_handler.error.is_some());
    assert!(resp_no_handler
        .error
        .as_ref()
        .unwrap()
        .contains("handler not registered"));
    assert_eq!(resp_no_handler.operation_id, "op_deadbeefcafebabe");
}

// --- 补充：register_handler 防御逻辑 -------------------------------------

#[test]
fn register_handler_rejects_malformed_operation_id() {
    let mut dispatcher = vos_core::hermes::HermesDispatcher::new();
    dispatcher
        .load_operation_artifact(&operation_fixture_json())
        .unwrap();

    let err = dispatcher
        .register_handler("bad_id", Box::new(vos_core::hermes::EchoHandler))
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("operation_id malformed"));
}

#[test]
fn register_handler_rejects_nonexistent_operation() {
    let mut dispatcher = vos_core::hermes::HermesDispatcher::new();
    dispatcher
        .load_operation_artifact(&operation_fixture_json())
        .unwrap();

    let err = dispatcher
        .register_handler(
            "op_0000000000000000",
            Box::new(vos_core::hermes::EchoHandler),
        )
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("not found in registry"));
}
