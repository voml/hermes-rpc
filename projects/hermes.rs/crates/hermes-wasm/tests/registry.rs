//! Registry and wire exports must delegate to `hermes-runtime`.

mod common;

use hermes_wasm::{HermesRegistry, decode_dispatch_request, encode_dispatch_response};
use serde_json::{Value, json};

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw).expect("export must return JSON")
}

#[test]
fn new_registry_is_empty() {
    let registry = HermesRegistry::new();
    assert_eq!(registry.service_count(), 0);
    assert_eq!(registry.operation_count(), 0);
    assert_eq!(parse(&registry.service_names()), json!([]));
}

#[test]
fn registers_contract_and_describes_method() {
    let mut registry = HermesRegistry::new();
    assert_eq!(parse(&registry.register_contract_artifact(&common::hermes_contract_json())), json!({ "ok": true }));
    assert_eq!(registry.service_count(), 1);
    assert_eq!(registry.operation_count(), 1);
    assert_eq!(parse(&registry.service_names()), json!([common::SERVICE_NAME]));

    let descriptor = parse(&registry.describe_method(common::SERVICE_NAME, common::METHOD_NAME));
    assert_eq!(descriptor["service"], common::SERVICE_NAME);
    assert_eq!(descriptor["method"], common::METHOD_NAME);
    assert_eq!(descriptor["methodId"], common::expected_method_id());
    assert_eq!(descriptor["operationId"], common::expected_operation_id());
    assert_eq!(descriptor["stream"], "unary");
    assert_eq!(descriptor["effect"], "read");
    assert_eq!(descriptor["requestType"], json!({ "scalar": "utf8" }));
}

#[test]
fn rejects_duplicate_and_non_hermes_registration() {
    let mut registry = HermesRegistry::new();
    registry.register_contract_artifact(&common::hermes_contract_json());

    let duplicate = parse(&registry.register_contract_artifact(&common::hermes_contract_json()));
    assert_eq!(duplicate["ok"], false);
    assert_eq!(duplicate["code"], "duplicate-registration");

    let iris = parse(&registry.register_contract_artifact(&common::iris_contract_json()));
    assert_eq!(iris["ok"], false);
    assert_eq!(iris["code"], "artifact-corrupt");
    assert_eq!(iris["diagnosticCode"], "ART010");
}

#[test]
fn reports_unknown_service_and_method() {
    let registry = HermesRegistry::new();
    let missing_service = parse(&registry.describe_method("Missing", common::METHOD_NAME));
    assert_eq!(missing_service["ok"], false);
    assert_eq!(missing_service["code"], "unknown-service");

    let mut registry = HermesRegistry::new();
    registry.register_contract_artifact(&common::hermes_contract_json());
    let missing_method = parse(&registry.describe_method(common::SERVICE_NAME, "write"));
    assert_eq!(missing_method["ok"], false);
    assert_eq!(missing_method["code"], "unknown-method");
}

#[test]
fn decodes_dispatch_request() {
    let decoded = parse(&decode_dispatch_request(r#"{"service":"FileService","method":"read","params":{"path":"a"}}"#));
    assert_eq!(decoded["service"], "FileService");
    assert_eq!(decoded["method"], "read");
    assert_eq!(decoded["params"], json!({ "path": "a" }));
}

#[test]
fn rejects_dispatch_request_with_unknown_or_missing_fields() {
    let unknown = parse(&decode_dispatch_request(r#"{"service":"FileService","method":"read","params":{},"extra":true}"#));
    assert_eq!(unknown["ok"], false);
    assert_eq!(unknown["code"], "invalid-request");

    let missing = parse(&decode_dispatch_request(r#"{"service":"FileService","method":"read"}"#));
    assert_eq!(missing["ok"], false);
    assert_eq!(missing["code"], "invalid-request");
}

#[test]
fn rejects_empty_dispatch_service_or_method() {
    let service = parse(&decode_dispatch_request(r#"{"service":"  ","method":"read","params":{}}"#));
    assert_eq!(service["ok"], false);
    assert_eq!(service["code"], "invalid-request");

    let method = parse(&decode_dispatch_request(r#"{"service":"FileService","method":"  ","params":{}}"#));
    assert_eq!(method["ok"], false);
    assert_eq!(method["code"], "invalid-request");
}

#[test]
fn encodes_dispatch_response() {
    let encoded = parse(&encode_dispatch_response(
        r#"{"service":"FileService","method":"read","operationId":"op_x","methodId":"mtd_x","result":{"path":"a"}}"#,
    ));
    assert_eq!(encoded["operationId"], "op_x");
    assert_eq!(encoded["result"], json!({ "path": "a" }));
    assert!(encoded.get("error").is_none());
}

#[test]
fn rejects_dispatch_response_with_unknown_field_or_bad_payload_count() {
    let unknown = parse(&encode_dispatch_response(r#"{"service":"FileService","method":"read","result":1,"extra":true}"#));
    assert_eq!(unknown["ok"], false);
    assert_eq!(unknown["code"], "invalid-request");

    let both = parse(&encode_dispatch_response(
        r#"{"service":"FileService","method":"read","result":1,"error":{"code":"internal","message":"boom"}}"#,
    ));
    assert_eq!(both["ok"], false);
    assert_eq!(both["code"], "invalid-request");

    let none = parse(&encode_dispatch_response(r#"{"service":"FileService","method":"read"}"#));
    assert_eq!(none["ok"], false);
    assert_eq!(none["code"], "invalid-request");
}
