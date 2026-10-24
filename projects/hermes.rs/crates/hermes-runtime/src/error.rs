//! RPC 领域错误分类。
//!
//! `vos_types::ArtifactError` 是上游 artifact 校验错误。RPC 领域需要稳定的错误码
//! 族，让 wire 错误体不必解析字符串，因此这里定义 `RpcError`。上游诊断码
//! （`ART010` / `ART020` / `ART021` / `OPC` / `SVC`）原样保留在
//! [`RpcError::diagnostic_code`] 里，Hermes 不另行编号。

use std::fmt;

use serde::{Deserialize, Serialize};
use vos_types::ArtifactError;

/// 稳定 RPC 错误码。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RpcErrorCode {
    /// artifact 无法解析、版本不支持或 identity 不一致。
    ArtifactCorrupt,
    /// 请求指向未注册的 service。
    UnknownService,
    /// 请求指向未注册的方法。
    UnknownMethod,
    /// 请求指向未注册的 operation。
    UnknownOperation,
    /// service 方法绑定的 operation 不在 payload 中。
    MissingOperationBinding,
    /// operation 存在但没有绑定 handler。
    HandlerNotRegistered,
    /// 同一 service / operation / handler 重复注册。
    DuplicateRegistration,
    /// 请求形状不合法（service / method 为空、参数无法解码等）。
    InvalidRequest,
    /// 合同类型没有声明 wire codec，无法安全编解码。
    UnsupportedCodec,
    /// handler 执行失败或返回值与声明的 canonical type 不符。
    HandlerFailed,
    /// handler 返回的形态与合同声明的 stream 模式不符。
    StreamModeMismatch,
    /// dispatch deadline 已过。
    DeadlineExceeded,
    /// dispatch 被取消。
    Cancelled,
    /// 内部错误（I/O、未分类错误）。
    Internal,
}

impl RpcErrorCode {
    /// 稳定的 wire 字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactCorrupt => "artifact-corrupt",
            Self::UnknownService => "unknown-service",
            Self::UnknownMethod => "unknown-method",
            Self::UnknownOperation => "unknown-operation",
            Self::MissingOperationBinding => "missing-operation-binding",
            Self::HandlerNotRegistered => "handler-not-registered",
            Self::DuplicateRegistration => "duplicate-registration",
            Self::InvalidRequest => "invalid-request",
            Self::UnsupportedCodec => "unsupported-codec",
            Self::HandlerFailed => "handler-failed",
            Self::StreamModeMismatch => "stream-mode-mismatch",
            Self::DeadlineExceeded => "deadline-exceeded",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for RpcErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// 携带稳定错误码的 RPC 错误。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpcError {
    code: RpcErrorCode,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    diagnostic_code: Option<String>,
}

impl RpcError {
    /// 用给定错误码和消息构造。
    pub fn new(code: RpcErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), diagnostic_code: None }
    }

    /// 附加上游诊断码，保持其原始拼写。
    pub fn with_diagnostic_code(mut self, diagnostic_code: impl Into<String>) -> Self {
        self.diagnostic_code = Some(diagnostic_code.into());
        self
    }

    /// 返回错误码。
    pub fn code(&self) -> RpcErrorCode {
        self.code
    }

    /// 返回错误消息。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 返回上游诊断码（若该错误来自 artifact 校验）。
    pub fn diagnostic_code(&self) -> Option<&str> {
        self.diagnostic_code.as_deref()
    }

    /// 把上游 artifact 校验错误归类到 RPC 错误码，并保留原始诊断码。
    pub fn from_artifact(err: ArtifactError) -> Self {
        Self::new(RpcErrorCode::ArtifactCorrupt, err.message).with_diagnostic_code(err.code)
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.diagnostic_code {
            Some(code) => write!(formatter, "{} [{}]: {}", self.code, code, self.message),
            None => write!(formatter, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for RpcError {}

impl From<ArtifactError> for RpcError {
    fn from(err: ArtifactError) -> Self {
        Self::from_artifact(err)
    }
}

impl From<std::io::Error> for RpcError {
    fn from(err: std::io::Error) -> Self {
        Self::new(RpcErrorCode::Internal, err.to_string())
    }
}

/// RPC 结果别名。
pub type RpcResult<T = ()> = Result<T, RpcError>;
