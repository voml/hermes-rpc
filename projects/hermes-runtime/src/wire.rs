//! dispatch 请求 / 响应 envelope。

use serde::{Deserialize, Serialize};

/// Hermes RPC 请求结构。实际 dispatch 也可以直接用原始字符串调用。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HermesDispatchRequest {
    /// `operation_id`（`op_` 开头）或 operation 名称。
    pub operation: String,
    /// 参数 JSON 对象。
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Hermes RPC 响应结构。result / error 二选一。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesDispatchResponse {
    /// 成功时的返回值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// 失败时的错误消息。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 本次 dispatch 最终命中的 `operation_id`（方便客户端回查）。
    pub operation_id: String,
}
