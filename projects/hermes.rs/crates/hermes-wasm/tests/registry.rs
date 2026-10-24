//! Registry and wire exports must delegate to `hermes-runtime`.

use hermes_wasm::{HermesRegistry, decode_dispatch_request, encode_dispatch_response};
use serde_json::Value;

const SERVICE_CONTRACT: &str = r#"{
  "service_name": "hermes.rpc.FileService",
  "methods": [
    { "name": "read", "request_type": "ReadRequest", "response_type": "ReadResponse", "docs": "" }
  ]
}"#;

const SERVICE_PROJECTION: &str = include_str!("fixtures/service_projection.json");

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw).expect("export must return JSON")
}

#[test]
fn new_registry_is_empty() {
    let registry = HermesRegistry::new();
    assert_eq!(registry.service_count(), 0);
    assert_eq!(parse(&registry.service_names()), serde_json::json!([]));
}

#[test]
fn registers_contract_and_describes_method() {
    let mut registry = HermesRegistry::new();
    assert_eq!(parse(&registry.register_service_contract(SERVICE_CONTRACT)), serde_json::json!({ "ok": true }));
    assert_eq!(registry.service_count(), 1);
    assert_eq!(parse(&registry.service_names()), serde_json::json!(["hermes.rpc.FileService"]));

    let descriptor = parse(&registry.describe_method("hermes.rpc.FileService", "read"));
    assert_eq!(descriptor["name"], "read");
    assert_eq!(descriptor["request_type"], "ReadRequest");
    assert_eq!(descriptor["response_type"], "ReadResponse");
}

#[test]
fn rejects_duplicate_service_registration() {
    let mut registry = HermesRegistry::new();
    assert_eq!(parse(&registry.register_service_contract(SERVICE_CONTRACT))["ok"], true);
    let duplicate = parse(&registry.register_service_contract(SERVICE_CONTRACT));
    assert_eq!(duplicate["ok"], false);
    assert_eq!(duplicate["code"], "duplicate-registration");
}

#[test]
fn reports_missing_service_and_method() {
    let registry = HermesRegistry::new();
    let missing_service = parse(&registry.describe_method("hermes.rpc.Missing", "read"));
    assert_eq!(missing_service["ok"], false);
    assert!(missing_service["message"].as_str().is_some_and(|message| message.contains("service not found")));

    let mut registry = HermesRegistry::new();
    registry.register_service_contract(SERVICE_CONTRACT);
    let missing_method = parse(&registry.describe_method("hermes.rpc.FileService", "write"));
    assert_eq!(missing_method["ok"], false);
    assert!(missing_method["message"].as_str().is_some_and(|message| message.contains("method not found")));
}

#[test]
fn registers_service_projection() {
    let mut registry = HermesRegistry::new();
    assert_eq!(parse(&registry.register_service_projection(SERVICE_PROJECTION)), serde_json::json!({ "ok": true }));
    assert_eq!(registry.service_count(), 1);
    assert_eq!(parse(&registry.service_names()), serde_json::json!(["demo.auth.IdentityService"]));
}

#[test]
fn rejects_malformed_service_projection() {
    let mut registry = HermesRegistry::new();
    let result = parse(&registry.register_service_projection(r#"{"envelope":{},"services":[]}"#));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "SRV010");
    assert_eq!(registry.service_count(), 0);
}

#[test]
fn decodes_dispatch_request() {
    let decoded = parse(&decode_dispatch_request(r#"{"operation":"op_0123456789abcdef","params":{"a":1}}"#));
    assert_eq!(decoded["operation"], "op_0123456789abcdef");
    assert_eq!(decoded["params"], serde_json::json!({ "a": 1 }));
}

#[test]
fn rejects_dispatch_request_with_unknown_field() {
    let result = parse(&decode_dispatch_request(r#"{"operation":"op_0123456789abcdef","extra":true}"#));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "invalid-request");
}

#[test]
fn rejects_empty_dispatch_operation() {
    let result = parse(&decode_dispatch_request(r#"{"operation":"  "}"#));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "invalid-request");
}

#[test]
fn encodes_dispatch_response() {
    let encoded =
        parse(&encode_dispatch_response(r#"{"result":{"value":1},"operationId":"op_0123456789abcdef"}"#));
    assert_eq!(encoded["operationId"], "op_0123456789abcdef");
    assert_eq!(encoded["result"], serde_json::json!({ "value": 1 }));
    assert!(encoded.get("error").is_none());
}

#[test]
fn rejects_dispatch_response_with_both_result_and_error() {
    let result = parse(&encode_dispatch_response(
        r#"{"result":1,"error":{"code":"internal","message":"boom"},"operationId":"op_0123456789abcdef"}"#,
    ));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "invalid-request");
}

#[test]
fn rejects_dispatch_response_without_payload() {
    let result = parse(&encode_dispatch_response(r#"{"operationId":"op_0123456789abcdef"}"#));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "invalid-request");
}
