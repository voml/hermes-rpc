//! `service.method` → operation → handler 路由与 dispatch。
//!
//! dispatch 是纯路由：handler 的业务逻辑由宿主用 Rust 实现，Hermes 只负责按
//! registry 解析出的 `OperationId` 找到 handler 并调用。VOS body 不进入本层，
//! 合同也不携带可执行代码。
//!
//! 请求参数按绑定的 `OperationContract.parameters` 校验，返回值按合同声明的
//! canonical type 校验。unary 与 server-stream 两种模式各自只接受对应的返回
//! 形态，模式不符返回 `stream-mode-mismatch`。
//!
//! 生命周期参数（deadline / cancellation / 并发上限）在 [`super::lifecycle`] 中
//! 定义，dispatch 前后各强制检查一次，等待并发许可期间同样响应取消与超时。

use std::collections::BTreeMap;

use serde_json::Value;
use vos_types::{MethodId, MethodStreamMode, OperationContract, OperationId};

use crate::{
    codec,
    contract::parse_contract_artifact,
    error::{RpcError, RpcErrorCode, RpcResult},
    lifecycle::{CallContext, CallOptions, InFlightLimiter},
    registry::{ContractRegistry, MethodRoute},
    wire::{HermesDispatchRequest, HermesDispatchResponse},
};

/// handler 返回给 dispatch 层的响应形态。
#[derive(Clone, Debug, PartialEq)]
pub enum OperationReply {
    /// unary 方法的单个返回值。
    Unary(Value),
    /// server-stream 方法的响应元素序列。
    ServerStream(Vec<Value>),
}

/// 宿主为每个 operation 注册的处理器 trait。
pub trait HermesOperationHandler: Send + Sync {
    /// 执行对应 operation。
    ///
    /// `ctx` 提供 deadline 与取消状态，长耗时 handler 应周期性调用
    /// [`CallContext::check`]。`operation` 是 registry 中绑定的
    /// [`OperationContract`]，`params` 是按合同参数 codec 解码后的 JSON 对象。
    fn execute(&self, ctx: &CallContext, operation: &OperationContract, params: &Value) -> RpcResult<OperationReply>;
}

/// 一次已校验的 dispatch 结果。
#[derive(Clone, Debug, PartialEq)]
pub enum DispatchReply {
    /// unary 返回值。
    Unary(Value),
    /// server-stream 响应元素序列。
    ServerStream(Vec<Value>),
}

/// Hermes RPC dispatch 核心：持有合同 registry + handler map。
#[derive(Default)]
pub struct HermesDispatcher {
    registry: ContractRegistry,
    handlers: BTreeMap<OperationId, Box<dyn HermesOperationHandler>>,
    limiter: InFlightLimiter,
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
    pub fn with_registry(registry: ContractRegistry) -> Self {
        Self { registry, handlers: BTreeMap::new(), limiter: InFlightLimiter::default() }
    }

    /// 只读访问内部 registry。
    pub fn registry(&self) -> &ContractRegistry {
        &self.registry
    }

    /// 可变访问内部 registry（绑定层逐份注册合同用）。
    pub fn registry_mut(&mut self) -> &mut ContractRegistry {
        &mut self.registry
    }

    /// 设置在途 dispatch 上限。`None` 或 `Some(0)` 表示不限。
    pub fn set_max_concurrency(&mut self, limit: Option<usize>) {
        self.limiter.set_limit(limit.filter(|limit| *limit > 0));
    }

    /// 当前在途 dispatch 上限。
    pub fn max_concurrency(&self) -> Option<usize> {
        self.limiter.limit()
    }

    /// 从 Hermes 合同 artifact JSON 解析并注册合同。
    pub fn load_contract_artifact(&mut self, json: &str) -> RpcResult<()> {
        let artifact = parse_contract_artifact(json)?;
        self.registry.register_artifact(&artifact)
    }

    /// 为 `operation_id` 注册一个 handler。
    ///
    /// 先按上游 identity 规则校验 id，再确认 registry 里确实存在该 operation，
    /// 提前暴露注册手误。
    pub fn register_handler(&mut self, operation_id: &str, handler: Box<dyn HermesOperationHandler>) -> RpcResult<()> {
        let operation_id = OperationId::parse(operation_id).map_err(|error| {
            RpcError::new(RpcErrorCode::InvalidRequest, format!("operation id malformed: {}", error.message))
        })?;
        if self.registry.operation(&operation_id).is_none() {
            return Err(RpcError::new(
                RpcErrorCode::UnknownOperation,
                format!("operation `{operation_id}` not found in registry — cannot register handler"),
            ));
        }
        if self.handlers.contains_key(&operation_id) {
            return Err(RpcError::new(
                RpcErrorCode::DuplicateRegistration,
                format!("handler for operation `{operation_id}` already registered"),
            ));
        }
        self.handlers.insert(operation_id, handler);
        Ok(())
    }

    /// 按 `service.method` 名查找对应的 `OperationId`，再注册 handler。
    ///
    /// 内部委托给 [`Self::register_handler`]，会走相同的 operation 存在性、
    /// 重复注册等校验逻辑。
    pub fn register_handler_by_method(
        &mut self,
        service_name: &str,
        method_name: &str,
        handler: Box<dyn HermesOperationHandler>,
    ) -> RpcResult<()> {
        let operation_id = self.registry.operation_for_method(service_name, method_name)?;
        self.register_handler(operation_id.as_str(), handler)
    }

    /// 按 durable `MethodId` 查找对应的 `OperationId`，再注册 handler。
    ///
    /// 内部委托给 [`Self::register_handler`]，走相同的 operation 存在性、
    /// 重复注册等校验逻辑。
    pub fn register_method_handler(
        &mut self,
        method_id: &MethodId,
        handler: Box<dyn HermesOperationHandler>,
    ) -> RpcResult<()> {
        let operation_id = self
            .registry
            .operation_id_for_method(method_id)
            .cloned()
            .ok_or_else(|| {
                RpcError::new(
                    RpcErrorCode::UnknownMethod,
                    format!("method `{method_id}` not found in registry — cannot register handler"),
                )
            })?;
        self.register_handler(operation_id.as_str(), handler)
    }

    /// 按 `service.method` dispatch，不设 deadline 也不绑定取消令牌。
    pub fn dispatch(&self, service: &str, method: &str, params: &Value) -> RpcResult<DispatchReply> {
        self.dispatch_with(service, method, params, &CallOptions::default())
    }

    /// 按 `service.method` dispatch，并带上生命周期参数。
    ///
    /// 顺序：解析 method 路由 → 找 handler → 等并发许可 → 校验参数 → 执行 →
    /// 校验返回值形态与类型。取消与 deadline 在每一步之间各检查一次；即使
    /// handler 已返回结果，只要期间超时或被取消，仍返回 `deadline-exceeded` /
    /// `cancelled`。
    pub fn dispatch_with(
        &self,
        service: &str,
        method: &str,
        params: &Value,
        options: &CallOptions,
    ) -> RpcResult<DispatchReply> {
        let route = self.registry.resolve(service, method)?;
        self.dispatch_route(&route, params, options)
    }

    fn dispatch_route(&self, route: &MethodRoute<'_>, params: &Value, options: &CallOptions) -> RpcResult<DispatchReply> {
        let ctx = CallContext::new(options.clone());
        ctx.check()?;

        let operation = route.operation;
        let handler = self.handlers.get(&operation.operation_id).ok_or_else(|| {
            RpcError::new(
                RpcErrorCode::HandlerNotRegistered,
                format!("handler not registered for operation {}", operation.operation_id),
            )
        })?;

        let _permit = self.limiter.acquire(&ctx)?;
        ctx.check()?;

        let decoded = codec::decode_params(operation, params)?;
        // handler panic 隔离：panic 被折算成 `internal` 错误，宿主线程与后续请求
        // 不受影响。错误分类仍只有 [`crate::error`] 一处真相源。
        let operation_id = operation.operation_id.clone();
        let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler.execute(&ctx, operation, &decoded)))
            .unwrap_or_else(|_| {
                Err(RpcError::new(
                    RpcErrorCode::Internal,
                    format!("handler for operation `{operation_id}` panicked"),
                ))
            });
        ctx.check()?;
        self.validate_reply(route, reply?)
    }

    fn validate_reply(&self, route: &MethodRoute<'_>, reply: OperationReply) -> RpcResult<DispatchReply> {
        match (route.method.stream, reply) {
            (MethodStreamMode::Unary, OperationReply::Unary(value)) => {
                let value = match &route.operation.result_type {
                    Some(result_type) => {
                        let codec = codec::codec_for(result_type)?;
                        codec::encode_value(&codec, &value)?
                    }
                    None => value,
                };
                Ok(DispatchReply::Unary(value))
            }
            (MethodStreamMode::ServerStream, OperationReply::ServerStream(items)) => {
                let codec = codec::codec_for(&route.method.response_type)?;
                let items = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        codec::check_value(&codec, item, &format!("response[{index}]"), RpcErrorCode::HandlerFailed)
                    })
                    .collect::<RpcResult<Vec<_>>>()?;
                Ok(DispatchReply::ServerStream(items))
            }
            (MethodStreamMode::Unary, OperationReply::ServerStream(_)) => Err(RpcError::new(
                RpcErrorCode::StreamModeMismatch,
                format!(
                    "method `{}.{}` is unary but the handler returned a server stream",
                    route.service.name, route.method.name
                ),
            )),
            (MethodStreamMode::ServerStream, OperationReply::Unary(_)) => Err(RpcError::new(
                RpcErrorCode::StreamModeMismatch,
                format!(
                    "method `{}.{}` is server-stream but the handler returned one value",
                    route.service.name, route.method.name
                ),
            )),
        }
    }

    /// 解析 `HermesDispatchRequest`。
    pub fn dispatch_request(&self, request: &HermesDispatchRequest) -> HermesDispatchResponse {
        self.dispatch_request_with(request, &CallOptions::default())
    }

    /// 解析 `HermesDispatchRequest` 并带上生命周期参数，结果折成 wire envelope。
    pub fn dispatch_request_with(&self, request: &HermesDispatchRequest, options: &CallOptions) -> HermesDispatchResponse {
        match self.registry.resolve(request.service(), request.method()) {
            Ok(route) => {
                let operation_id = route.operation.operation_id.to_string();
                let method_id = route.method.method_id.to_string();
                let result = self.dispatch_route(&route, request.params(), options);
                HermesDispatchResponse::from_reply(
                    request.service().to_string(),
                    request.method().to_string(),
                    operation_id,
                    method_id,
                    result,
                )
            }
            Err(error) => {
                HermesDispatchResponse::from_error(request.service().to_string(), request.method().to_string(), error)
            }
        }
    }
}
