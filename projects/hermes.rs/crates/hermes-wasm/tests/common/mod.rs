//! 合同 fixture 构造，供 `tests/` 下各测试文件共用。

// 每个测试 target 只使用其中一部分 helper，未用到的 helper 不算 dead code。
#![allow(dead_code)]

use hermes_runtime::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, MethodId, MethodIdentityKey,
    MethodStreamMode, OperationContract, OperationEffect, OperationId, OperationIdentityKey, OperationKind,
    OperationParameterContract, OperationParameterKey, ResolvedCanonicalType, ServiceContract, ServiceId, ServiceIdentityKey,
    ServiceMethodContract,
};

pub const SERVICE_NAME: &str = "FileService";
pub const METHOD_NAME: &str = "read";

fn utf8() -> ResolvedCanonicalType {
    ResolvedCanonicalType::Scalar(CanonicalScalar::Utf8)
}

pub fn operation() -> OperationContract {
    let parameters = vec![OperationParameterContract { name: "path".to_string(), canonical_type: utf8() }];
    let identity = OperationIdentityKey::new(
        &["demo".to_string(), "auth".to_string()],
        OperationKind::Query,
        "read",
        vec![OperationParameterKey::new("path", "utf8").expect("parameter key")],
        "utf8",
    )
    .expect("operation identity");
    OperationContract {
        operation_id: OperationId::derive(&identity),
        kind: OperationKind::Query,
        name: "read".to_string(),
        canonical_path: identity.canonical_path.clone(),
        parameters,
        cardinality: OperationContract::cardinality_of(&Some(utf8())),
        result_type: Some(utf8()),
        effect: OperationEffect::Read,
        capabilities: Vec::new(),
        source_unit_id: 0,
        span: (0..16usize).into(),
        stable_id_hint: None,
    }
}

pub fn service(operation: &OperationContract) -> ServiceContract {
    let canonical_path = vec!["demo".to_string(), "auth".to_string(), SERVICE_NAME.to_string()];
    let service_id = ServiceId::derive(&ServiceIdentityKey::new(canonical_path.clone()).expect("service identity"));
    let method_identity = MethodIdentityKey::new(service_id.clone(), METHOD_NAME).expect("method identity");
    ServiceContract {
        service_id,
        name: SERVICE_NAME.to_string(),
        canonical_path,
        attributes: Vec::new(),
        methods: vec![ServiceMethodContract {
            method_id: MethodId::derive(&method_identity),
            name: METHOD_NAME.to_string(),
            request_type: utf8(),
            response_type: utf8(),
            stream: MethodStreamMode::Unary,
            operation_id: operation.operation_id.clone(),
            attributes: Vec::new(),
            stable_id_hint: None,
        }],
        source_unit_id: 0,
        span: (0..32usize).into(),
        stable_id_hint: None,
    }
}

pub fn artifact(format: ContractFormat) -> ContractArtifact {
    let operation = operation();
    let service_contract = service(&operation);
    ContractArtifact::seal(
        format,
        ContractProducer::new("hermes-tests", "0"),
        ContractPayload { operations: vec![operation], services: vec![service_contract] },
    )
}

pub fn hermes_contract_json() -> String {
    artifact(ContractFormat::Hermes).to_json().expect("Hermes artifact serializes")
}

pub fn iris_contract_json() -> String {
    artifact(ContractFormat::Iris).to_json().expect("Iris artifact serializes")
}

pub fn expected_method_id() -> String {
    service(&operation()).methods[0].method_id.to_string()
}

pub fn expected_operation_id() -> String {
    operation().operation_id.to_string()
}
