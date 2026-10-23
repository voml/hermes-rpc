//! operation → handler 路由与 dispatch。
//!
//! dispatch 层是纯路由：handler 内部的业务逻辑由用户用 Rust 写死，Hermes 只
//! 负责按 `operation_id` 找到 handler 并调用。VOS projection 阶段尚未把 body
//! lowering 成可执行代码。

use std::collections::BTreeMap;

use vos_core::projection::{HermesOperationArtifact, HermesOperationProjection, HermesServiceArtifact, OPERATION_ID_PREFIX};

use crate::{
    error::{RpcError, RpcErrorCode, RpcResult},
    loader::JsonServiceLoader,
    registry::{BTreeServiceRegistry, HermesServiceRegistry},
    wire::{HermesDispatchRequest, HermesDispatchResponse},
};

/// 用户为每个 operation 注册的处理器 trait。
pub trait HermesOperationHandler: Send + Sync {
    /// 执行对应 operation，返回 JSON 值或结构化错误。
    ///
    /// `projection` 提供 operation 的静态声明信息（name / kind /
    /// parameters 等），`params` 是调用方传过来的 JSON 参数对象。
    fn execute(&self, projection: &HermesOperationProjection, params: &serde_json::Value) -> RpcResult<serde_json::Value>;
}

/// 校验 `operation_id` 字符串形状：必须是 `"op_"` + 恰好 16 个 hex 数字。
fn is_valid_operation_id(id: &str) -> bool {
    if !id.starts_with(OPERATION_ID_PREFIX) {
        return false;
    }
    let tail = &id[OPERATION_ID_PREFIX.len()..];
    tail.len() == 16 && tail.chars().all(|c| c.is_ascii_hexdigit())
}

/// Hermes RPC dispatch 核心：持有 registry + handler map，按
/// `operation_id` 路由到用户注册的 `HermesOperationHandler`。
#[derive(Default)]
pub struct HermesDispatcher {
    registry: BTreeServiceRegistry,
    handlers: BTreeMap<String, Box<dyn HermesOperationHandler>>,
}

impl Clone for HermesDispatcher {
    fn clone(&self) -> Self {
        // handler map 里的 opaque trait object 不能 Clone，新实例保留
        // 相同 registry 但 handler 列表为空——调用方可重新注册。
        Self { registry: self.registry.clone(), handlers: BTreeMap::new() }
    }
}

impl std::fmt::Debug for HermesDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HermesDispatcher")
            .field("registry", &self.registry)
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl HermesDispatcher {
    /// 构造一个空的 dispatcher。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用已有 registry 构造 dispatcher。
    pub fn with_registry(registry: BTreeServiceRegistry) -> Self {
        Self { registry, handlers: BTreeMap::new() }
    }

    /// 返回一个共享的 `JsonServiceLoader` 实例（无状态，调用零成本）。
    pub fn loader(&self) -> JsonServiceLoader {
        JsonServiceLoader::default()
    }

    /// 只读访问内部 registry。
    pub fn registry(&self) -> &BTreeServiceRegistry {
        &self.registry
    }

    /// 从 JSON 字符串解析 `HermesServiceArtifact` 并注册到内部 registry。
    pub fn load_service_artifact(&mut self, json: &str) -> RpcResult<()> {
        let artifact = HermesServiceArtifact::from_json(json).map_err(RpcError::from_artifact)?;
        self.registry.register_artifact(&artifact)
    }

    /// 从 JSON 字符串解析 `HermesOperationArtifact` 并注册到内部 registry。
    pub fn load_operation_artifact(&mut self, json: &str) -> RpcResult<()> {
        let artifact = HermesOperationArtifact::from_json(json).map_err(RpcError::from_artifact)?;
        self.registry.register_operation_artifact(&artifact)
    }

    /// 为 `operation_id` 注册一个 handler。
    ///
    /// 会先校验 `operation_id` 形状（`op_` + 16 hex），再校验 registry
    /// 里确实存在这个 operation——提前暴露注册手误，防止 dispatch
    /// 才发现。
    pub fn register_handler(&mut self, operation_id: &str, handler: Box<dyn HermesOperationHandler>) -> RpcResult<()> {
        if !is_valid_operation_id(operation_id) {
            return Err(RpcError::new(RpcErrorCode::InvalidRequest, format!("operation_id malformed: {operation_id}")));
        }
        if self.registry.find_operation(operation_id).is_none() {
            return Err(RpcError::new(
                RpcErrorCode::UnknownOperation,
                format!("operation `{operation_id}` not found in registry — cannot register handler"),
            ));
        }
        if self.handlers.contains_key(operation_id) {
            return Err(RpcError::new(
                RpcErrorCode::DuplicateRegistration,
                format!("handler for operation `{operation_id}` already registered"),
            ));
        }
        self.handlers.insert(operation_id.to_string(), handler);
        Ok(())
    }

    /// 按 `operation_id` 直接 dispatch。
    ///
    /// 未知 operation 返回 `unknown-operation`，未绑定 handler 返回
    /// `handler-not-registered`，handler 自身的错误原样透传。
    pub fn dispatch(&self, operation_id: &str, params: &serde_json::Value) -> RpcResult<serde_json::Value> {
        let projection = self
            .registry
            .find_operation(operation_id)
            .ok_or_else(|| RpcError::new(RpcErrorCode::UnknownOperation, format!("operation not found: {operation_id}")))?;
        let handler = self.handlers.get(operation_id).ok_or_else(|| {
            RpcError::new(RpcErrorCode::HandlerNotRegistered, format!("handler not registered for operation {operation_id}"))
        })?;
        handler.execute(projection, params)
    }

    /// 按 operation 名称 dispatch。先在 registry 里按 `name` 找 operation，
    /// 拿到 `operation_id` 后走 `dispatch`。
    pub fn dispatch_by_name(&self, name: &str, params: &serde_json::Value) -> RpcResult<serde_json::Value> {
        let projection = self
            .registry
            .find_operation_by_name(name)
            .ok_or_else(|| RpcError::new(RpcErrorCode::UnknownOperation, format!("operation not found by name: {name}")))?;
        self.dispatch(&projection.operation_id, params)
    }

    /// 解析 `HermesDispatchRequest`：如果 `operation` 字段以 `op_` 开头
    /// 视为 `operation_id`，否则视为 operation 名称。
    pub fn dispatch_request(&self, request: &HermesDispatchRequest) -> HermesDispatchResponse {
        let is_id = is_valid_operation_id(&request.operation);
        let (result, operation_id) = if is_id {
            (self.dispatch(&request.operation, &request.params), request.operation.clone())
        }
        else {
            match self.registry.find_operation_by_name(&request.operation) {
                Some(op) => (self.dispatch(&op.operation_id, &request.params), op.operation_id.clone()),
                None => (
                    Err(RpcError::new(
                        RpcErrorCode::UnknownOperation,
                        format!("operation not found by name: {}", request.operation),
                    )),
                    String::new(),
                ),
            }
        };
        match result {
            Ok(value) => HermesDispatchResponse { result: Some(value), error: None, operation_id },
            Err(err) => HermesDispatchResponse { result: None, error: Some(err), operation_id },
        }
    }
}
