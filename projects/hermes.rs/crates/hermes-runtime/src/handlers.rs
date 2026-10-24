//! 内建测试 handler。
//!
//! 这些 handler 只服务于 fixture 级测试，不承载任何业务语义。

use serde_json::Value;
use vos_types::OperationContract;

use crate::{
    dispatcher::{HermesOperationHandler, OperationReply},
    error::{RpcError, RpcErrorCode, RpcResult},
    lifecycle::CallContext,
};

/// 把 `params` 原样返回的测试 handler。
#[derive(Clone, Copy, Debug, Default)]
pub struct EchoHandler;

impl HermesOperationHandler for EchoHandler {
    fn execute(&self, _ctx: &CallContext, _operation: &OperationContract, params: &Value) -> RpcResult<OperationReply> {
        Ok(OperationReply::Unary(params.clone()))
    }
}

/// 把 `params.a + params.b` 相加返回的测试 handler。
///
/// `a` / `b` 的 VOS 类型是 `i64`，wire codec 声明为 JSON string，因此期望
/// `{"a": "3", "b": "4"}` → 返回 `"7"`。若参数缺失或不是整数字符串，返回
/// `invalid-request`。
#[derive(Clone, Copy, Debug, Default)]
pub struct AddHandler;

impl HermesOperationHandler for AddHandler {
    fn execute(&self, _ctx: &CallContext, _operation: &OperationContract, params: &Value) -> RpcResult<OperationReply> {
        let a = int_param(params, "a")?;
        let b = int_param(params, "b")?;
        Ok(OperationReply::Unary(Value::String((a + b).to_string())))
    }
}

fn int_param(params: &Value, name: &str) -> RpcResult<i64> {
    params
        .get(name)
        .and_then(Value::as_str)
        .and_then(|text| text.parse::<i64>().ok())
        .ok_or_else(|| RpcError::new(RpcErrorCode::InvalidRequest, format!("missing or non-integer-string param `{name}`")))
}
