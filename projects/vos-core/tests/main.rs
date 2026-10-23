//! Hermes 接口骨架的 mock 验证测试。
//!
//! 这些测试不做真实 RPC 运行时，只验证 `HermesServiceLoader` /
//! `HermesServiceRegistry` trait 能被 mock 实现、artifact 能被 JSON loader
//! 解析、registry 能正确注册和查找服务。

use vos_core::hermes::{
    BTreeServiceRegistry, HermesMethodDescriptor, HermesServiceContract,
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
