//! VOS artifact 合同镜像的解码与校验测试。
//!
//! 只覆盖 `vos-core` 的 projection 层：service / operation projection 的 serde
//! 对齐、版本拒绝、span 边界，以及 `svc_` identity 复算。loader / registry /
//! dispatch 的测试在 `hermes-runtime`。

use vos_core::{
    error::{ErrorKind, QError},
    projection::{HermesOperationArtifact, HermesOperationKind, HermesServiceArtifact, compute_service_id},
};

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
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/service_projection.basic.json")
}

#[test]
fn load_artifact_from_file_parses_vos_projection_fixture() {
    let bytes = std::fs::read(fixture_path()).expect("fixture exists");
    let artifact = HermesServiceArtifact::from_slice(&bytes).expect("should parse projection fixture");

    assert_eq!(artifact.envelope.stage, "service-syntax-projection");
    assert_eq!(artifact.envelope.contract_format_version, "vos-service-contract-v0");
    assert_eq!(artifact.envelope.language_version, "vos-language-v0");
    assert_eq!(artifact.envelope.source_units.len(), 1);
    assert_eq!(artifact.envelope.source_units[0].source_unit_id, 0);
    assert_eq!(artifact.services.len(), 2);
}

#[test]
fn load_artifact_from_bytes_parses_vos_projection() {
    let bytes = std::fs::read(fixture_path()).expect("fixture exists");
    let artifact = HermesServiceArtifact::from_slice(&bytes).expect("should parse projection from bytes");

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
fn artifact_version_mismatch_is_rejected() {
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
        "services": [{"name": "A", "canonicalPath": ["A"], "serviceId": "svc_9473795796e84dbb", "span": {"start": 0, "end": 10}}]
    }"#;

    let err = HermesServiceArtifact::from_slice(bad).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("unsupported service artifact version"));
}

#[test]
fn artifact_rejects_unknown_fields() {
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
        "services": [{"name": "A", "canonicalPath": ["A"], "serviceId": "svc_9473795796e84dbb", "span": {"start": 0, "end": 10}}]
    }"#;

    let err = HermesServiceArtifact::from_slice(with_extra).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Json, "deny_unknown_fields 应将其视为 JSON 错误");
}

#[test]
fn artifact_roundtrip_via_to_json_from_json() {
    let bytes = std::fs::read(fixture_path()).expect("fixture exists");
    let artifact = HermesServiceArtifact::from_slice(&bytes).expect("fixture parses");

    let json = artifact.to_json().expect("serialize");
    let roundtrip = HermesServiceArtifact::from_json(&json).expect("roundtrip");

    assert_eq!(roundtrip, artifact);
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
            {"name": "A", "canonicalPath": ["A"], "serviceId": "svc_9473795796e84dbb", "span": {"start": 0, "end": 999}}
        ]
    }"#;

    let err = HermesServiceArtifact::from_slice(bad).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("service span out of bounds"), "got: {}", err.message());
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
            {"name": "A", "canonicalPath": ["A"], "serviceId": "svc_9473795796e84dbb", "span": {"start": 10, "end": 2}}
        ]
    }"#;

    let err = HermesServiceArtifact::from_slice(bad).unwrap_err();
    // deserialize_span 直接返回 serde 错误
    assert!(err.kind() == ErrorKind::Json || err.kind() == ErrorKind::Validation, "unexpected error kind: {:?}", err.kind());
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
            {"name": "A", "canonicalPath": ["A"], "serviceId": "svc_9473795796e84dbb", "span": {"start": 0, "end": 12},
             "body": {"text": "body", "span": {"start": 20, "end": 50}}}
        ]
    }"#;

    let err = HermesServiceArtifact::from_slice(bad).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("service body span out of bounds"), "got: {}", err.message());
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
            {{"name": "A", "canonicalPath": ["A"], "serviceId": "svc_9473795796e84dbb", "span": {{"start": 0, "end": {len}}}}}
        ]
    }}"#,
        source = source.replace('"', "\\\""),
        len = source.len(),
    );

    let artifact = HermesServiceArtifact::from_json(&json).expect("char-boundary span should be accepted");
    assert_eq!(artifact.services.len(), 1);
}

// --- 上游 service projection 合同样例 ------------------------------------

fn upstream_service_fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/service_projection.upstream_basic.json")
}

#[test]
fn loads_upstream_service_projection_with_service_id() {
    let bytes = std::fs::read(upstream_service_fixture_path()).expect("upstream fixture exists");
    let artifact = HermesServiceArtifact::from_slice(&bytes).expect("upstream service projection must decode");

    assert_eq!(artifact.services.len(), 1);
    let service = &artifact.services[0];
    assert_eq!(service.name, "IdentityService");
    assert_eq!(service.canonical_path, vec!["demo", "auth", "IdentityService"]);
    assert_eq!(service.service_id, "svc_3c7addc7a558968f");
    assert_eq!(
        service.service_id,
        compute_service_id(&service.canonical_path, &service.name),
        "service id 必须可由 canonical path 与 name 复算"
    );
}

#[test]
fn service_id_mismatch_is_rejected() {
    let bad = br#"{
        "envelope": {
            "stage": "service-syntax-projection",
            "contractFormatVersion": "vos-service-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "sourceUnits": [{"sourceUnitId": 0, "source": "service A {}"}]
        },
        "services": [{
            "name": "A",
            "canonicalPath": ["A"],
            "serviceId": "svc_0000000000000000",
            "span": {"start": 0, "end": 10}
        }]
    }"#;

    let err = HermesServiceArtifact::from_slice(bad).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("does not match canonical path"));
}

#[test]
fn malformed_service_id_is_rejected() {
    let bad = br#"{
        "envelope": {
            "stage": "service-syntax-projection",
            "contractFormatVersion": "vos-service-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "sourceUnits": [{"sourceUnitId": 0, "source": "service A {}"}]
        },
        "services": [{
            "name": "A",
            "canonicalPath": ["A"],
            "serviceId": "svc_xyz",
            "span": {"start": 0, "end": 10}
        }]
    }"#;

    let err = HermesServiceArtifact::from_slice(bad).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("service id is malformed"));
}

// --- HermesOperationArtifact fixture 测试 --------------------------------

fn operation_fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/operation_projection.basic.json")
}

#[test]
fn load_operation_artifact_from_file_parses_fixture() {
    let bytes = std::fs::read(operation_fixture_path()).expect("fixture exists");
    let artifact = HermesOperationArtifact::from_slice(&bytes).expect("should parse operation fixture");

    assert_eq!(artifact.envelope.stage, "operation-syntax-projection");
    assert_eq!(artifact.envelope.contract_format_version, "vos-operation-contract-v0");
    assert_eq!(artifact.operations.len(), 3);
}

#[test]
fn load_operation_artifact_parses_kind_name_and_span() {
    let bytes = std::fs::read(operation_fixture_path()).expect("fixture exists");
    let artifact = HermesOperationArtifact::from_slice(&bytes).expect("should parse from bytes");

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
    let bytes = std::fs::read(operation_fixture_path()).expect("fixture exists");
    let artifact = HermesOperationArtifact::from_slice(&bytes).expect("fixture parses");

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

    let err = HermesOperationArtifact::from_slice(bad).unwrap_err();
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

    let err = HermesOperationArtifact::from_slice(bad).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert!(err.message().contains("operation id is malformed"));
}
