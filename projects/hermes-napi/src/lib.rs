//! N-API binding for Hermes RPC dispatch.
//!
//! Exposes [`HermesDispatcher`] core methods to Node.js via `napi-rs`.
//! Handler registration stays on the Rust side (user must build hermes-napi
//! with their own handler impls); this layer only handles artifact loading
//! and dispatch routing.
//!
//! Errors cross the boundary in two channels: a `NapiError` for load-time
//! failures, and the structured `error` field of [`HermesDispatchJsResult`]
//! for dispatch results. Both carry a stable RPC error code.

use hermes_runtime::{
    CallOptions, HermesDispatchRequest, HermesDispatchResponse, HermesDispatcher,
    error::{RpcError, RpcErrorCode, RpcResult},
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
    /// Human-readable detail. Clients must branch on `code`, not on this text.
    pub message: String,
}

/// Dispatch result returned to JS.
#[napi(object)]
#[derive(Clone, Debug)]
pub struct HermesDispatchJsResult {
    /// Operation id that handled the dispatch.
    pub operation_id: String,
    /// Success result JSON string, or `None` on error.
    pub result: Option<String>,
    /// Structured error, or `None` on success.
    pub error: Option<HermesRpcErrorJs>,
}

fn rpc_error_to_js(err: &RpcError) -> HermesRpcErrorJs {
    HermesRpcErrorJs { code: err.code().as_str().to_string(), message: err.message().to_string() }
}

fn response_to_js(resp: HermesDispatchResponse) -> HermesDispatchJsResult {
    HermesDispatchJsResult {
        operation_id: resp.operation_id,
        result: resp.result.map(|v| v.to_string()),
        error: resp.error.as_ref().map(rpc_error_to_js),
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

    /// Load a `HermesServiceArtifact` JSON string into the dispatcher registry.
    #[napi]
    pub fn load_service_artifact(&mut self, json: String) -> std::result::Result<(), NapiError> {
        map_rpc(self.inner.load_service_artifact(&json), "load_service_artifact")
    }

    /// Load a `HermesOperationArtifact` JSON string into the dispatcher registry.
    #[napi]
    pub fn load_operation_artifact(&mut self, json: String) -> std::result::Result<(), NapiError> {
        map_rpc(self.inner.load_operation_artifact(&json), "load_operation_artifact")
    }

    /// Set the in-flight dispatch limit. `null` or `0` means unlimited.
    #[napi]
    pub fn set_max_concurrency(&mut self, limit: Option<u32>) {
        self.inner.set_max_concurrency(limit.map(|limit| limit as usize));
    }

    /// Dispatch by operation id (`op_<hex>`) or by name.
    ///
    /// `params_json` must be a valid JSON object. `timeout_ms` sets the dispatch
    /// deadline: when the handler outlives it, the structured error carries
    /// `deadline-exceeded`. Dispatch failures are encoded in the structured
    /// `error` field of [`HermesDispatchJsResult`].
    #[napi]
    pub fn dispatch(&self, operation: String, params_json: String, timeout_ms: Option<u32>) -> HermesDispatchJsResult {
        let params = match decode_params(&params_json) {
            Ok(params) => params,
            Err(error) => return HermesDispatchJsResult { operation_id: String::new(), result: None, error: Some(error) },
        };
        let request = HermesDispatchRequest { operation, params };
        response_to_js(self.inner.dispatch_request_with(&request, &call_options(timeout_ms)))
    }
}

// ---------------------------------------------------------------------------
// Single-shot convenience: build dispatcher → load both artifacts → dispatch
// ---------------------------------------------------------------------------

/// Load both artifacts and dispatch in one call.
///
/// Useful for simple "script and dispatch" flows that don't need a persistent
/// dispatcher handle. Artifact loading errors propagate as `NapiError` with an
/// RPC error code; dispatch results are returned in the response object.
#[napi]
pub fn run_hermes_dispatch(
    service_artifact_json: String,
    operation_artifact_json: String,
    operation: String,
    params_json: String,
    timeout_ms: Option<u32>,
) -> std::result::Result<HermesDispatchJsResult, NapiError> {
    let mut dispatcher = HermesDispatcher::new();
    map_rpc(dispatcher.load_service_artifact(&service_artifact_json), "load_service_artifact")?;
    map_rpc(dispatcher.load_operation_artifact(&operation_artifact_json), "load_operation_artifact")?;
    let params = match decode_params(&params_json) {
        Ok(params) => params,
        Err(error) => return Ok(HermesDispatchJsResult { operation_id: String::new(), result: None, error: Some(error) }),
    };
    let request = HermesDispatchRequest { operation, params };
    Ok(response_to_js(dispatcher.dispatch_request_with(&request, &call_options(timeout_ms))))
}
