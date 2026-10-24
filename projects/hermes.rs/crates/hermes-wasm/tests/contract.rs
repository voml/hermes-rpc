//! Envelope exports must delegate to the shared contract loader and keep the
//! upstream diagnostic codes unchanged.

mod common;

use hermes_wasm::{hermes_contract_format, validate_contract_artifact};
use serde_json::Value;

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw).expect("export must return JSON")
}

#[test]
fn exports_the_only_accepted_contract_format() {
    assert_eq!(hermes_contract_format(), "vos/hermes.contract/v0");
}

#[test]
fn accepts_a_hermes_contract_artifact() {
    let result = parse(&validate_contract_artifact(&common::hermes_contract_json()));
    assert_eq!(result, serde_json::json!({ "ok": true }));
}

#[test]
fn rejects_an_iris_contract_artifact() {
    let result = parse(&validate_contract_artifact(&common::iris_contract_json()));
    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "artifact-corrupt");
    assert_eq!(result["diagnosticCode"], "ART010");
}

#[test]
fn rejects_unknown_contract_versions_and_fields() {
    let mut value: Value = serde_json::from_str(&common::hermes_contract_json()).expect("fixture JSON");
    value["version"] = serde_json::json!("vos/hermes.contract/v1");
    let unknown_version = parse(&validate_contract_artifact(&value.to_string()));
    assert_eq!(unknown_version["diagnosticCode"], "ART010");

    let mut value: Value = serde_json::from_str(&common::hermes_contract_json()).expect("fixture JSON");
    value["futureField"] = serde_json::json!(true);
    let unknown_field = parse(&validate_contract_artifact(&value.to_string()));
    assert_eq!(unknown_field["diagnosticCode"], "ART010");
}

#[test]
fn rejects_fingerprint_mismatch_and_missing_producer_version() {
    let mut value: Value = serde_json::from_str(&common::hermes_contract_json()).expect("fixture JSON");
    value["fingerprint"] = serde_json::json!("0".repeat(64));
    let mismatch = parse(&validate_contract_artifact(&value.to_string()));
    assert_eq!(mismatch["diagnosticCode"], "ART020");

    let mut value: Value = serde_json::from_str(&common::hermes_contract_json()).expect("fixture JSON");
    value["producer"]["version"] = serde_json::json!("");
    let missing_producer = parse(&validate_contract_artifact(&value.to_string()));
    assert_eq!(missing_producer["diagnosticCode"], "ART021");
}
