//! N-API binding for Hermes RPC dispatch.
//!
//! Exposes [`HermesDispatcher`] core methods to Node.js via `napi-rs`.
//! Handler registration stays on the Rust side (user must build hermes-napi
//! with their own handler impls); this layer only handles artifact loading
//! and dispatch routing.

use hermes_runtime::{HermesDispatchRequest, HermesDispatchResponse, HermesDispatcher, error::HermesError};
use napi::{Error as NapiError, Status};
use napi_derive::napi;
use serde_json::Value;

/// Hermes N-API version.
#[napi]
pub fn hermes_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Dispatch result returned to JS.
#[napi(object)]
#[derive(Clone, Debug)]
pub struct HermesDispatchJsResult {
    /// Operation id that handled the dispatch.
    pub operation_id: String,
    /// Success result JSON string, or `None` on error.
    pub result: Option<String>,
    /// Error message, or `None` on success.
    pub error: Option<String>,
}

fn response_to_js(resp: HermesDispatchResponse) -> HermesDispatchJsResult {
    HermesDispatchJsResult { operation_id: resp.operation_id, result: resp.result.map(|v| v.to_string()), error: resp.error }
}

fn map_hermes<T>(res: Result<T, HermesError>, ctx: &str) -> std::result::Result<T, NapiError> {
    res.map_err(|e| NapiError::new(Status::GenericFailure, format!("{ctx}: {e}")))
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
        map_hermes(self.inner.load_service_artifact(&json), "load_service_artifact")
    }

    /// Load a `HermesOperationArtifact` JSON string into the dispatcher registry.
    #[napi]
    pub fn load_operation_artifact(&mut self, json: String) -> std::result::Result<(), NapiError> {
        map_hermes(self.inner.load_operation_artifact(&json), "load_operation_artifact")
    }

    /// Dispatch by operation id (`op_<hex>`) or by name.
    ///
    /// `params_json` must be a valid JSON object. Returns result JSON string.
    /// Error state is encoded in the response (see [`HermesDispatchJsResult`]);
    /// a missing handler returns `Result` variant with an `error` field.
    #[napi]
    pub fn dispatch(&self, operation: String, params_json: String) -> HermesDispatchJsResult {
        let params: Value = serde_json::from_str(&params_json).unwrap_or_default();
        let request = HermesDispatchRequest { operation, params };
        response_to_js(self.inner.dispatch_request(&request))
    }
}

// ---------------------------------------------------------------------------
// Single-shot convenience: build dispatcher → load both artifacts → dispatch
// ---------------------------------------------------------------------------

/// Load both artifacts and dispatch in one call.
///
/// Useful for simple "script and dispatch" flows that don't need a persistent
/// dispatcher handle. Artifact loading errors propagate as NAPI errors;
/// dispatch result is returned in the response object.
#[napi]
pub fn run_hermes_dispatch(
    service_artifact_json: String,
    operation_artifact_json: String,
    operation: String,
    params_json: String,
) -> std::result::Result<HermesDispatchJsResult, NapiError> {
    let mut dispatcher = HermesDispatcher::new();
    map_hermes(dispatcher.load_service_artifact(&service_artifact_json), "load_service_artifact")?;
    map_hermes(dispatcher.load_operation_artifact(&operation_artifact_json), "load_operation_artifact")?;
    let params: Value = serde_json::from_str(&params_json).unwrap_or_default();
    let request = HermesDispatchRequest { operation, params };
    Ok(response_to_js(dispatcher.dispatch_request(&request)))
}
