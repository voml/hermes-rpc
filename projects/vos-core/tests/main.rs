//! Hermes 接口骨架的 mock 验证测试。
//!
//! 这些测试不做真实 RPC 运行时，只验证 `HermesServiceLoader` /
//! `HermesServiceRegistry` trait 能被 mock 实现、artifact 能被 JSON loader
//! 解析、registry 能正确注册和查找服务。
//!
//! 同时覆盖 Hermes 对 `vos-language` `ServiceProjectionArtifact` 的本地镜像
//! 解析：`HermesServiceArtifact` 与上游 JSON 形状必须完全兼容。

use vos_core::hermes::{
    BTreeServiceRegistry, HermesMethodDescriptor, HermesServiceArtifact,
    HermesServiceContract, HermesServiceLoader, HermesServiceRegistry,
    JsonServiceLoader,
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
