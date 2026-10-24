//! dispatch 的 wire envelope。
//!
//! 请求只携带 `service` + `method` + `params`：调用方不需要自己拼 method 或
//! operation 的 durable id，路由完全由 registry 解析。响应回填解析出的
//! `operationId` / `methodId` 便于调用方做日志与追踪。
//!
//! unary 结果放 `result`，server-stream 结果放 `stream`，失败放 `error`；三者
//! 互斥，由 [`HermesDispatchResponse::payload_count`] 暴露给绑定层做形状闸门。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    dispatcher::DispatchReply,
    error::{RpcError, RpcResult},
};

/// 一次 dispatch 请求。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HermesDispatchRequest {
    service: String,
    method: String,
    params: Value,
}

impl HermesDispatchRequest {
    /// 构造请求。
    pub fn new(service: impl Into<String>, method: impl Into<String>, params: Value) -> Self {
        Self { service: service.into(), method: method.into(), params }
    }

    /// service 名。
    pub fn service(&self) -> &str {
        &self.service
    }

    /// method 名。
    pub fn method(&self) -> &str {
        &self.method
    }

    /// wire 参数对象。
    pub fn params(&self) -> &Value {
        &self.params
    }

    /// 结构闸门：service 与 method 都必须非空。
    pub fn validate(&self) -> RpcResult<()> {
        if self.service.trim().is_empty() {
            return Err(RpcError::new(crate::error::RpcErrorCode::InvalidRequest, "service must not be empty"));
        }
        if self.method.trim().is_empty() {
            return Err(RpcError::new(crate::error::RpcErrorCode::InvalidRequest, "method must not be empty"));
        }
        Ok(())
    }
}

/// 一次 dispatch 响应。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HermesDispatchResponse {
    service: String,
    method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    method_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stream: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

impl HermesDispatchResponse {
    /// 成功响应：unary 结果或 server-stream 元素序列。
    pub fn from_reply(
        service: String,
        method: String,
        operation_id: String,
        method_id: String,
        reply: RpcResult<DispatchReply>,
    ) -> Self {
        match reply {
            Ok(DispatchReply::Unary(result)) => Self {
                service,
                method,
                operation_id: Some(operation_id),
                method_id: Some(method_id),
                result: Some(result),
                stream: None,
                error: None,
            },
            Ok(DispatchReply::ServerStream(items)) => Self {
                service,
                method,
                operation_id: Some(operation_id),
                method_id: Some(method_id),
                result: None,
                stream: Some(items),
                error: None,
            },
            Err(error) => Self {
                service,
                method,
                operation_id: Some(operation_id),
                method_id: Some(method_id),
                result: None,
                stream: None,
                error: Some(error),
            },
        }
    }

    /// 失败响应：路由尚未解析出 durable id 时两者均为 `None`。
    pub fn from_error(service: String, method: String, error: RpcError) -> Self {
        Self { service, method, operation_id: None, method_id: None, result: None, stream: None, error: Some(error) }
    }

    /// service 名。
    pub fn service(&self) -> &str {
        &self.service
    }

    /// method 名。
    pub fn method(&self) -> &str {
        &self.method
    }

    /// 解析出的 operation durable id。
    pub fn operation_id(&self) -> Option<&str> {
        self.operation_id.as_deref()
    }

    /// 解析出的 method durable id。
    pub fn method_id(&self) -> Option<&str> {
        self.method_id.as_deref()
    }

    /// unary 结果。
    pub fn result(&self) -> Option<&Value> {
        self.result.as_ref()
    }

    /// server-stream 元素。
    pub fn stream(&self) -> Option<&[Value]> {
        self.stream.as_deref()
    }

    /// 错误。
    pub fn error(&self) -> Option<&RpcError> {
        self.error.as_ref()
    }

    /// `result` / `stream` / `error` 中已填充的项数，成功与失败都必须是 1。
    pub fn payload_count(&self) -> usize {
        usize::from(self.result.is_some()) + usize::from(self.stream.is_some()) + usize::from(self.error.is_some())
    }
}
