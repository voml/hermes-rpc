//! 内建测试 handler。
//!
//! 这些 handler 只服务于 fixture 级测试，不承载任何业务语义。

use vos_core::{
    error::{HermesError, QResult},
    projection::HermesOperationProjection,
};

use crate::dispatcher::HermesOperationHandler;

/// 把 `params` 原样返回的测试 handler。
#[derive(Clone, Copy, Debug, Default)]
pub struct EchoHandler;

impl HermesOperationHandler for EchoHandler {
    fn execute(&self, _projection: &HermesOperationProjection, params: &serde_json::Value) -> QResult<serde_json::Value> {
        Ok(params.clone())
    }
}

/// 把 `params.a + params.b` 相加返回的测试 handler。
///
/// 期望 JSON：`{"a": 3, "b": 4}` → 返回 `7`。若参数缺失或不是数值，返回
/// `ErrorKind::Validation`。
#[derive(Clone, Copy, Debug, Default)]
pub struct AddHandler;

impl HermesOperationHandler for AddHandler {
    fn execute(&self, _projection: &HermesOperationProjection, params: &serde_json::Value) -> QResult<serde_json::Value> {
        let a = params
            .get("a")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| HermesError::new(vos_core::error::ErrorKind::Validation, "missing or non-integer param `a`"))?;
        let b = params
            .get("b")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| HermesError::new(vos_core::error::ErrorKind::Validation, "missing or non-integer param `b`"))?;
        Ok(serde_json::Value::Number(serde_json::Number::from(a + b)))
    }
}
