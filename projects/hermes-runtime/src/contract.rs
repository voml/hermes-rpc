//! Hermes 内部的服务 / 方法占位描述。
//!
//! method 合同归上游 VOS 语言标准（`vos-language` 的 service projection）。
//! 上游 method 级 projection 交付前，`HermesMethodDescriptor` 只是 registry 的
//! 占位形状，不构成按 `service.method` 路由的能力。

use serde::{Deserialize, Serialize};

/// Hermes 端内部的服务描述。
///
/// 真实消费主链是 `vos_core::projection::HermesServiceArtifact`（与上游
/// `ServiceProjectionArtifact` 对齐）。本结构体是 registry 的存储形状。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HermesServiceContract {
    /// 服务的全局唯一名称（例如 `"hermes.rpc.FileService"`）。
    pub service_name: String,
    /// 服务暴露的所有方法。
    pub methods: Vec<HermesMethodDescriptor>,
    /// artifact 元信息（来源版本、revision 等）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

/// 单个 RPC 方法的占位描述。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HermesMethodDescriptor {
    /// 方法名。
    pub name: String,
    /// 请求体的类型名（对应 VOS schema 中的 type）。
    pub request_type: String,
    /// 响应体的类型名。
    pub response_type: String,
    /// 可选的方法文档字符串。
    #[serde(default)]
    pub docs: String,
}
