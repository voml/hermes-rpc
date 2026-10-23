//! operation → handler 路由与 dispatch。
//!
//! dispatch 层是纯路由：handler 内部的业务逻辑由用户用 Rust 写死，Hermes 只
//! 负责按 `operation_id` 找到 handler 并调用。VOS projection 阶段尚未把 body
//! lowering 成可执行代码。
//!
//! 生命周期参数（deadline / cancellation / 并发上限）在 [`super::lifecycle`] 中
//! 定义，dispatch 前后各强制检查一次，等待并发许可期间同样响应取消与超时。

use std::collections::BTreeMap;

use vos_core::projection::{HermesOperationArtifact, HermesOperationProjection, HermesServiceArtifact, OPERATION_ID_PREFIX};

use crate::{
    error::{RpcError, RpcErrorCode, RpcResult},
    lifecycle::{CallContext, CallOptions, InFlightLimiter},
    loader::JsonServiceLoader,
    registry::{BTreeServiceRegistry, HermesServiceRegistry},
    wire::{HermesDispatchRequest, HermesDispatchResponse},
};

/// 用户为每个 operation 注册的处理器 trait。
pub trait HermesOperationHandler: Send + Sync {
    /// 执行对应 operation，返回 JSON 值或结构化错误。
    ///
    /// `ctx` 提供 deadline 与取消状态，长耗时 handler 应周期性调用
    /// [`CallContext::check`]。`projection` 提供 operation 的静态声明信息
    /// （name / kind / parameters 等），`params` 是调用方传过来的 JSON 参数对象。
    fn execute(
        &self,
        ctx: &CallContext,
        projection: &HermesOperationProjection,
        params: &serde_json::Value,
    ) -> RpcResult<serde_json::Value>;
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
    limiter: InFlightLimiter,
}

impl Clone for HermesDispatcher {
    fn clone(&self) -> Self {
        // handler map 里的 opaque trait object 不能 Clone，新实例保留
        // 相同 registry 与并发上限但 handler 列表为空——调用方可重新注册。
        Self { registry: self.registry.clone(), handlers: BTreeMap::new(), limiter: self.limiter.clone() }
    }
}

impl std::fmt::Debug for HermesDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HermesDispatcher")
            .field("registry", &self.registry)
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .field("max_concurrency", &self.limiter.limit())
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
        Self { registry, handlers: BTreeMap::new(), limiter: InFlightLimiter::default() }
    }

    /// 返回一个共享的 `JsonServiceLoader` 实例（无状态，调用零成本）。
    pub fn loader(&self) -> JsonServiceLoader {
        JsonServiceLoader::default()
    }

    /// 只读访问内部 registry。
    pub fn registry(&self) -> &BTreeServiceRegistry {
        &self.registry
    }

    /// 设置在途 dispatch 上限。`None` 或 `Some(0)` 表示不限。
    pub fn set_max_concurrency(&mut self, limit: Option<usize>) {
        self.limiter.set_limit(limit.filter(|limit| *limit > 0));
    }

    /// 当前在途 dispatch 上限。
    pub fn max_concurrency(&self) -> Option<usize> {
        self.limiter.limit()
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

    /// 按 `operation_id` 直接 dispatch，不设 deadline 也不绑定取消令牌。
    ///
    /// 未知 operation 返回 `unknown-operation`，未绑定 handler 返回
    /// `handler-not-registered`，handler 自身的错误原样透传。
    pub fn dispatch(&self, operation_id: &str, params: &serde_json::Value) -> RpcResult<serde_json::Value> {
        self.dispatch_with(operation_id, params, &CallOptions::default())
    }

    /// 按 `operation_id` dispatch，并带上生命周期参数。
    ///
    /// 顺序：解析 operation → 找 handler → 等并发许可 → handler 执行。
    /// 取消与 deadline 在每一步之间各检查一次；即使 handler 已返回结果，只要
    /// 期间超时或被取消，仍返回 `deadline-exceeded` / `cancelled`，避免调用方
    /// 拿到已经过期的响应。
    pub fn dispatch_with(
        &self,
        operation_id: &str,
        params: &serde_json::Value,
        options: &CallOptions,
    ) -> RpcResult<serde_json::Value> {
        let ctx = CallContext::new(options.clone());
        ctx.check()?;

        let projection = self
            .registry
            .find_operation(operation_id)
            .ok_or_else(|| RpcError::new(RpcErrorCode::UnknownOperation, format!("operation not found: {operation_id}")))?;
        let handler = self.handlers.get(operation_id).ok_or_else(|| {
            RpcError::new(RpcErrorCode::HandlerNotRegistered, format!("handler not registered for operation {operation_id}"))
        })?;

        let _permit = self.limiter.acquire(&ctx)?;
        ctx.check()?;

        let result = handler.execute(&ctx, projection, params);
        ctx.check()?;
        result
    }

    /// 按 operation 名称 dispatch，不设 deadline 也不绑定取消令牌。
    pub fn dispatch_by_name(&self, name: &str, params: &serde_json::Value) -> RpcResult<serde_json::Value> {
        self.dispatch_by_name_with(name, params, &CallOptions::default())
    }

    /// 按 operation 名称 dispatch，并带上生命周期参数。先在 registry 里按
    /// `name` 找 operation，拿到 `operation_id` 后走 [`Self::dispatch_with`]。
    pub fn dispatch_by_name_with(
        &self,
        name: &str,
        params: &serde_json::Value,
        options: &CallOptions,
    ) -> RpcResult<serde_json::Value> {
        let projection = self
            .registry
            .find_operation_by_name(name)
            .ok_or_else(|| RpcError::new(RpcErrorCode::UnknownOperation, format!("operation not found by name: {name}")))?;
        self.dispatch_with(&projection.operation_id, params, options)
    }

    /// 解析 `HermesDispatchRequest`：如果 `operation` 字段以 `op_` 开头
    /// 视为 `operation_id`，否则视为 operation 名称。
    pub fn dispatch_request(&self, request: &HermesDispatchRequest) -> HermesDispatchResponse {
        self.dispatch_request_with(request, &CallOptions::default())
    }

    /// 解析 `HermesDispatchRequest` 并带上生命周期参数，结果折成 wire envelope。
    pub fn dispatch_request_with(&self, request: &HermesDispatchRequest, options: &CallOptions) -> HermesDispatchResponse {
        let is_id = is_valid_operation_id(&request.operation);
        let (result, operation_id) = if is_id {
            (self.dispatch_with(&request.operation, &request.params, options), request.operation.clone())
        }
        else {
            match self.registry.find_operation_by_name(&request.operation) {
                Some(op) => (self.dispatch_with(&op.operation_id, &request.params, options), op.operation_id.clone()),
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
