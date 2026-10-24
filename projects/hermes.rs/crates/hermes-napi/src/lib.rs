//! N-API binding for Hermes RPC dispatch.
//!
//! Exposes [`hermes_runtime::HermesDispatcher`] core methods to Node.js via
//! `napi-rs`. Handler registration stays on the Rust side (user must build
//! hermes-napi with their own handler impls); this layer only handles contract
//! artifact loading and `service.method` dispatch routing.
//!
//! Errors cross the boundary in two channels: a `NapiError` for load-time
//! failures, and the structured `error` field of [`HermesDispatchJsResult`]
//! for dispatch results. Both carry a stable RPC error code; upstream
//! diagnostics (`ART010` / `ART020` / `ART021` / `OPC` / `SVC`) stay in
//! `diagnosticCode`.

use hermes_runtime::{
    CallOptions, ContractArtifact, HermesDispatchRequest, HermesDispatchResponse, HermesDispatcher, RpcError, RpcErrorCode,
    RpcResult,
};
use napi::{Error as NapiError, Status};
use napi_derive::napi;
use serde_json::Value;
use std::time::Duration;

/// Hermes N-API version.
#[napi]
pub fn hermes_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Structured RPC error returned to JS.
#[napi(object)]
#[derive(Clone, Debug)]
pub struct HermesRpcErrorJs {
    /// Stable RPC error code (kebab-case), see `RpcErrorCode`.
    pub code: String,
    /// Upstream artifact diagnostic code (`ART010`, `ART020`, `SVC016`, ...).
    pub diagnostic_code: Option<String>,
    /// Human-readable detail. Clients must branch on `code`, not on this text.
    pub message: String,
}

/// Dispatch result returned to JS.
#[napi(object)]
#[derive(Clone, Debug)]
pub struct HermesDispatchJsResult {
    /// Service that was addressed.
    pub service: String,
    /// Method that was addressed.
    pub method: String,
    /// Operation id resolved by the registry.
    pub operation_id: Option<String>,
    /// Method id resolved by the registry.
    pub method_id: Option<String>,
    /// Success result JSON string for unary methods, or `None`.
    pub result: Option<String>,
    /// Response element JSON strings for server-stream methods, or `None`.
    pub stream: Option<Vec<String>>,
    /// Structured error, or `None` on success.
    pub error: Option<HermesRpcErrorJs>,
}

fn rpc_error_to_js(err: &RpcError) -> HermesRpcErrorJs {
    HermesRpcErrorJs {
        code: err.code().as_str().to_string(),
        diagnostic_code: err.diagnostic_code().map(str::to_string),
        message: err.message().to_string(),
    }
}

fn response_to_js(resp: HermesDispatchResponse) -> HermesDispatchJsResult {
    HermesDispatchJsResult {
        service: resp.service().to_string(),
        method: resp.method().to_string(),
        operation_id: resp.operation_id().map(str::to_string),
        method_id: resp.method_id().map(str::to_string),
        result: resp.result().map(Value::to_string),
        stream: resp.stream().map(|items| items.iter().map(Value::to_string).collect()),
        error: resp.error().map(rpc_error_to_js),
    }
}

fn map_rpc<T>(res: RpcResult<T>, ctx: &str) -> std::result::Result<T, NapiError> {
    res.map_err(|e| NapiError::new(Status::GenericFailure, format!("{ctx} [{}]: {}", e.code().as_str(), e.message())))
}

/// Decode `params_json`, mapping invalid JSON into a structured
/// `invalid-request` failure instead of silently defaulting to `null`.
fn decode_params(params_json: &str) -> std::result::Result<Value, HermesRpcErrorJs> {
    serde_json::from_str(params_json).map_err(|e| {
        rpc_error_to_js(&RpcError::new(RpcErrorCode::InvalidRequest, format!("params_json is not valid JSON: {e}")))
    })
}

/// Build dispatch lifecycle options from an optional millisecond timeout.
fn call_options(timeout_ms: Option<u32>) -> CallOptions {
    match timeout_ms {
        Some(ms) => CallOptions::with_timeout(Duration::from_millis(u64::from(ms))),
        None => CallOptions::new(),
    }
}

fn decode_contract(json: &str) -> std::result::Result<ContractArtifact, NapiError> {
    hermes_runtime::parse_contract_artifact(json).map_err(|e| {
        NapiError::new(
            Status::GenericFailure,
            format!("contract artifact [{}]: {}", e.diagnostic_code().unwrap_or("unknown"), e.message()),
        )
    })
}

fn empty_result(service: &str, method: &str, error: HermesRpcErrorJs) -> HermesDispatchJsResult {
    HermesDispatchJsResult {
        service: service.to_string(),
        method: method.to_string(),
        operation_id: None,
        method_id: None,
        result: None,
        stream: None,
        error: Some(error),
    }
}

/// Opaque Hermes dispatcher handle exposed to JS.
#[napi]
pub struct HermesDispatcherHandle {
    inner: HermesDispatcher,
}

#[napi]
impl HermesDispatcherHandle {
    /// Create a new empty dispatcher.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: HermesDispatcher::new() }
    }

    /// Load a `vos/hermes.contract/v0` artifact JSON string into the registry.
    #[napi]
    pub fn load_contract_artifact(&mut self, json: String) -> std::result::Result<(), NapiError> {
        map_rpc(self.inner.load_contract_artifact(&json), "load_contract_artifact")
    }

    /// Set the in-flight dispatch limit. `null` or `0` means unlimited.
    #[napi]
    pub fn set_max_concurrency(&mut self, limit: Option<u32>) {
        self.inner.set_max_concurrency(limit.map(|limit| limit as usize));
    }

    /// Dispatch `service.method` with a JSON parameter object.
    ///
    /// `params_json` must decode to a JSON object matching the bound
    /// `OperationContract.parameters`. `timeout_ms` sets the dispatch deadline:
    /// when the handler outlives it, the structured error carries
    /// `deadline-exceeded`. Dispatch failures are encoded in the structured
    /// `error` field of [`HermesDispatchJsResult`].
    #[napi]
    pub fn dispatch(
        &self,
        service: String,
        method: String,
        params_json: String,
        timeout_ms: Option<u32>,
    ) -> HermesDispatchJsResult {
        let params = match decode_params(&params_json) {
            Ok(params) => params,
            Err(error) => return empty_result(&service, &method, error),
        };
        let request = HermesDispatchRequest::new(service, method, params);
        response_to_js(self.inner.dispatch_request_with(&request, &call_options(timeout_ms)))
    }
}

// ---------------------------------------------------------------------------
// Single-shot convenience: parse artifact → dispatch in one call
// ---------------------------------------------------------------------------

/// Parse a contract artifact and dispatch `service.method` in one call.
///
/// Useful for simple "script and dispatch" flows that don't need a persistent
/// dispatcher handle. Artifact loading errors propagate as `NapiError` with the
/// upstream diagnostic code; dispatch results are returned in the response
/// object.
#[napi]
pub fn run_hermes_dispatch(
    contract_artifact_json: String,
    service: String,
    method: String,
    params_json: String,
    timeout_ms: Option<u32>,
) -> std::result::Result<HermesDispatchJsResult, NapiError> {
    let artifact = decode_contract(&contract_artifact_json)?;
    let mut dispatcher = HermesDispatcher::new();
    map_rpc(dispatcher.registry_mut().register_artifact(&artifact), "register_contract_artifact")?;
    let params = match decode_params(&params_json) {
        Ok(params) => params,
        Err(error) => return Ok(empty_result(&service, &method, error)),
    };
    let request = HermesDispatchRequest::new(service, method, params);
    Ok(response_to_js(dispatcher.dispatch_request_with(&request, &call_options(timeout_ms))))
}
