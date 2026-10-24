//! Contract validation exports must reuse the runtime loaders.

use hermes_wasm::{hermes_version, validate_service_contract, validate_service_projection};
use serde_json::Value;

const SERVICE_CONTRACT: &str = r#"{
  "service_name": "hermes.rpc.FileService",
  "methods": [
    {
      "name": "read",
      "request_type": "ReadRequest",
      "response_type": "ReadResponse",
      "docs": "Read one file."
    }
  ],
  "revision": "rev-1"
}"#;

const SERVICE_PROJECTION: &str = include_str!("fixtures/service_projection.json");

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw).expect("export must return JSON")
}

#[test]
fn version_matches_crate_version() {
    assert_eq!(hermes_version(), env!("CARGO_PKG_VERSION"));
    assert!(!hermes_version().is_empty());
}

#[test]
fn accepts_valid_service_contract() {
    assert_eq!(parse(&validate_service_contract(SERVICE_CONTRACT)), serde_json::json!({ "ok": true }));
}

#[test]
fn rejects_service_contract_with_wrong_field_type() {
    let result = parse(&validate_service_contract(r#"{"service_name":42,"methods":[]}"#));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "invalid-request");
    assert!(result["message"].as_str().is_some_and(|message| !message.is_empty()));
}

#[test]
fn rejects_truncated_service_contract() {
    let result = parse(&validate_service_contract(r#"{"service_name":"a""#));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "invalid-request");
}

#[test]
fn accepts_valid_service_projection() {
    assert_eq!(parse(&validate_service_projection(SERVICE_PROJECTION)), serde_json::json!({ "ok": true }));
}

#[test]
fn rejects_service_projection_with_unknown_stage() {
    let mut artifact = parse(SERVICE_PROJECTION);
    artifact["envelope"]["stage"] = Value::String("resolved-contract".to_string());
    let result = parse(&validate_service_projection(&artifact.to_string()));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "SRV011");
}

#[test]
fn rejects_service_projection_with_unknown_field() {
    let mut artifact = parse(SERVICE_PROJECTION);
    artifact["services"][0]["unexpected"] = Value::Bool(true);
    let result = parse(&validate_service_projection(&artifact.to_string()));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "SRV010");
}
