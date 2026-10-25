//! Browser WebAssembly surface for the Hermes RPC runtime.
//!
//! Every export delegates to `hermes-runtime` or upstream `vos-types`: the
//! binding adds no RPC semantics of its own. JSON in, JSON out keeps the
//! TypeScript host free of a parallel artifact or wire parser.
//!
//! Errors cross the boundary as structured JSON:
//! `{"ok": false, "code": "<stable-code>", "diagnosticCode"?: "<upstream-code>",
//! "message": "<detail>"}`.

#![deny(missing_docs)]

use hermes_runtime::{
    ContractArtifact, ContractRegistry, HERMES_CONTRACT_FORMAT, HermesContractLoader, HermesDispatchRequest,
    HermesDispatchResponse, JsonContractLoader, MethodRoute, RpcError,
};
use wasm_bindgen::prelude::*;

fn ok_json() -> String {
    r#"{"ok":true}"#.to_string()
}

fn failure_from(error: &RpcError) -> String {
    let mut value = serde_json::json!({
        "ok": false,
        "code": error.code().as_str(),
        "message": error.message(),
    });
    if let Some(diagnostic_code) = error.diagnostic_code() {
        value["diagnosticCode"] = serde_json::Value::String(diagnostic_code.to_string());
    }
    value.to_string()
}

fn failure_json(code: &str, message: impl Into<String>) -> String {
    serde_json::json!({ "ok": false, "code": code, "message": message.into() }).to_string()
}

fn encode_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|err| failure_json("internal", err.to_string()))
}

fn reject_unknown_keys(json: &str, allowed: &[&str]) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|err| err.to_string())?;
    let object = value.as_object().ok_or_else(|| "expected a JSON object".to_string())?;
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("unknown field `{key}`")),
        None => Ok(()),
    }
}

fn route_json(route: &MethodRoute<'_>) -> String {
    serde_json::json!({
        "service": route.service.name,
        "method": route.method.name,
        "methodId": route.method.method_id,
        "operationId": route.operation.operation_id,
        "requestType": route.method.request_type,
        "responseType": route.method.response_type,
        "stream": route.method.stream,
        "effect": route.operation.effect,
        "capabilities": route.operation.capabilities,
        "parameters": route.operation.parameters,
    })
    .to_string()
}

/// Hermes WASM binding version (Cargo package version).
#[wasm_bindgen(js_name = hermesVersion)]
pub fn hermes_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Contract format accepted by Hermes.
#[wasm_bindgen(js_name = hermesContractFormat)]
pub fn hermes_contract_format() -> String {
    HERMES_CONTRACT_FORMAT.to_string()
}

/// Load and validate a `vos/hermes.contract/v0` artifact JSON document.
#[wasm_bindgen(js_name = validateContractArtifact)]
pub fn validate_contract_artifact(json: &str) -> String {
    match JsonContractLoader.load_contract(json.as_bytes()) {
        Ok(_) => ok_json(),
        Err(err) => failure_from(&err),
    }
}

/// Decode a `HermesDispatchRequest` and return its canonical JSON form.
#[wasm_bindgen(js_name = decodeDispatchRequest)]
pub fn decode_dispatch_request(json: &str) -> String {
    let request: HermesDispatchRequest = match serde_json::from_str(json) {
        Ok(request) => request,
        Err(err) => return failure_json("invalid-request", err.to_string()),
    };
    if let Err(err) = request.validate() {
        return failure_from(&err);
    }
    encode_json(&request)
}

/// Decode a `HermesDispatchResponse` and return its canonical JSON form.
#[wasm_bindgen(js_name = encodeDispatchResponse)]
pub fn encode_dispatch_response(json: &str) -> String {
    let allowed = ["service", "method", "operationId", "methodId", "result", "stream", "error"];
    if let Err(message) = reject_unknown_keys(json, &allowed) {
        return failure_json("invalid-request", message);
    }
    let response: HermesDispatchResponse = match serde_json::from_str(json) {
        Ok(response) => response,
        Err(err) => return failure_json("invalid-request", err.to_string()),
    };
    if response.service().trim().is_empty() || response.method().trim().is_empty() {
        return failure_json("invalid-request", "dispatch response must name its `service` and `method`");
    }
    if response.payload_count() != 1 {
        return failure_json("invalid-request", "dispatch response must carry exactly one of `result`, `stream` or `error`");
    }
    if let Some(operation_id) = response.operation_id()
        && operation_id.trim().is_empty()
    {
        return failure_json("invalid-request", "dispatch response `operationId` must not be empty");
    }
    if let Some(method_id) = response.method_id()
        && method_id.trim().is_empty()
    {
        return failure_json("invalid-request", "dispatch response `methodId` must not be empty");
    }
    encode_json(&response)
}

/// Contract registry exposed to JavaScript hosts.
#[wasm_bindgen]
#[derive(Default)]
pub struct HermesRegistry {
    inner: ContractRegistry,
}

#[wasm_bindgen]
impl HermesRegistry {
    /// Create an empty registry.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `vos/hermes.contract/v0` artifact JSON document.
    pub fn register_contract_artifact(&mut self, json: &str) -> String {
        let contract: ContractArtifact = match JsonContractLoader.load_contract(json.as_bytes()) {
            Ok(contract) => contract,
            Err(err) => return failure_from(&err),
        };
        match self.inner.register_artifact(&contract) {
            Ok(()) => ok_json(),
            Err(err) => failure_from(&err),
        }
    }

    /// Number of registered services.
    pub fn service_count(&self) -> u32 {
        self.inner.service_count() as u32
    }

    /// Number of registered operations.
    pub fn operation_count(&self) -> u32 {
        self.inner.operation_count() as u32
    }

    /// Registered service names as a JSON array.
    pub fn service_names(&self) -> String {
        encode_json(&self.inner.service_names())
    }

    /// Describe one registered method, or return an error JSON document.
    pub fn describe_method(&self, service: &str, method: &str) -> String {
        match self.inner.resolve(service, method) {
            Ok(route) => route_json(&route),
            Err(err) => failure_from(&err),
        }
    }
}
