//! Browser WebAssembly surface for the Hermes RPC runtime.
//!
//! Every export delegates to `hermes-runtime` or upstream `vos-types`: the
//! binding adds no RPC semantics of its own. JSON in, JSON out keeps the
//! TypeScript host free of a parallel artifact or wire parser.
//!
//! Errors cross the boundary as structured JSON:
//! `{"ok": false, "code": "<stable-code>", "message": "<detail>"}`.

#![deny(missing_docs)]

use hermes_runtime::{
    BTreeServiceRegistry, HermesDispatchRequest, HermesDispatchResponse, HermesServiceContract,
    HermesServiceLoader, HermesServiceRegistry, JsonServiceLoader,
};
use wasm_bindgen::prelude::*;

fn ok_json() -> String {
    r#"{"ok":true}"#.to_string()
}

fn failure_json(code: &str, message: impl Into<String>) -> String {
    serde_json::json!({ "ok": false, "code": code, "message": message.into() }).to_string()
}

fn encode_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|err| failure_json("internal", err.to_string()))
}

/// Hermes WASM binding version (Cargo package version).
#[wasm_bindgen(js_name = hermesVersion)]
pub fn hermes_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Validate a hand-written `HermesServiceContract` JSON document.
#[wasm_bindgen(js_name = validateServiceContract)]
pub fn validate_service_contract(json: &str) -> String {
    match JsonServiceLoader.load(json.as_bytes()) {
        Ok(_) => ok_json(),
        Err(err) => failure_json(err.code().as_str(), err.message()),
    }
}

/// Validate an upstream `vos_types::ServiceProjectionArtifact` JSON document.
#[wasm_bindgen(js_name = validateServiceProjection)]
pub fn validate_service_projection(json: &str) -> String {
    match vos_types::ServiceProjectionArtifact::from_json(json) {
        Ok(_) => ok_json(),
        Err(err) => failure_json(&err.code, err.message),
    }
}

/// Decode a `HermesDispatchRequest` and return its canonical JSON form.
#[wasm_bindgen(js_name = decodeDispatchRequest)]
pub fn decode_dispatch_request(json: &str) -> String {
    let request: HermesDispatchRequest = match serde_json::from_str(json) {
        Ok(request) => request,
        Err(err) => return failure_json("invalid-request", err.to_string()),
    };
    if request.operation.trim().is_empty() {
        return failure_json("invalid-request", "dispatch request `operation` must not be empty");
    }
    encode_json(&request)
}

/// Decode a `HermesDispatchResponse` and return its canonical JSON form.
#[wasm_bindgen(js_name = encodeDispatchResponse)]
pub fn encode_dispatch_response(json: &str) -> String {
    let response: HermesDispatchResponse = match serde_json::from_str(json) {
        Ok(response) => response,
        Err(err) => return failure_json("invalid-request", err.to_string()),
    };
    if response.result.is_some() == response.error.is_some() {
        return failure_json("invalid-request", "dispatch response must carry exactly one of `result` or `error`");
    }
    if response.operation_id.trim().is_empty() {
        return failure_json("invalid-request", "dispatch response `operationId` must not be empty");
    }
    encode_json(&response)
}

/// Service registry exposed to JavaScript hosts.
#[wasm_bindgen]
#[derive(Default)]
pub struct HermesRegistry {
    inner: BTreeServiceRegistry,
}

#[wasm_bindgen]
impl HermesRegistry {
    /// Create an empty registry.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `HermesServiceContract` JSON document.
    pub fn register_service_contract(&mut self, json: &str) -> String {
        let contract: HermesServiceContract = match JsonServiceLoader.load(json.as_bytes()) {
            Ok(contract) => contract,
            Err(err) => return failure_json(err.code().as_str(), err.message()),
        };
        match self.inner.register(contract) {
            Ok(()) => ok_json(),
            Err(err) => failure_json(err.code().as_str(), err.message()),
        }
    }

    /// Register every service in an upstream `ServiceProjectionArtifact` JSON document.
    pub fn register_service_projection(&mut self, json: &str) -> String {
        let artifact = match vos_types::ServiceProjectionArtifact::from_json(json) {
            Ok(artifact) => artifact,
            Err(err) => return failure_json(&err.code, err.message),
        };
        match self.inner.register_artifact(&artifact) {
            Ok(()) => ok_json(),
            Err(err) => failure_json(err.code().as_str(), err.message()),
        }
    }

    /// Number of registered services.
    pub fn service_count(&self) -> u32 {
        self.inner.all().len() as u32
    }

    /// Registered service names as a JSON array.
    pub fn service_names(&self) -> String {
        let names: Vec<&str> = self.inner.all().iter().map(|contract| contract.service_name.as_str()).collect();
        encode_json(&names)
    }

    /// Describe one registered method, or return an error JSON document.
    pub fn describe_method(&self, service: &str, method: &str) -> String {
        if self.inner.find(service).is_none() {
            return failure_json("invalid-request", format!("service not found: {service}"));
        }
        match self.inner.method(service, method) {
            Some(descriptor) => encode_json(descriptor),
            None => failure_json("invalid-request", format!("method not found: {service}.{method}")),
        }
    }
}
