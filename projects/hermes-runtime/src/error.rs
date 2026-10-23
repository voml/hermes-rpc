//! RPC 领域错误分类。
//!
//! `vos_core::error::HermesError` 是语言层与 artifact 校验的通用错误。RPC 领域
//! 需要稳定的错误码族，让 wire 错误体不必解析字符串，因此这里定义 `RpcError`。
//! artifact 层错误在 crate 边界统一转换，不向上泄漏 `ErrorKind`。

use std::fmt;

use serde::{Deserialize, Serialize};
use vos_core::error::{ErrorKind, HermesError};

/// 稳定 RPC 错误码。
///
/// wire 表达是 kebab-case 字符串，客户端按码分支，不解析消息文本。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RpcErrorCode {
    /// artifact 无法解析、版本不支持或 identity 不一致。
    ArtifactCorrupt,
    /// 请求指向未注册的 operation。
    UnknownOperation,
    /// operation 存在但没有绑定 handler。
    HandlerNotRegistered,
    /// 同一 service / operation / handler 重复注册。
    DuplicateRegistration,
    /// 请求形状不合法（operation id 畸形、参数无法解码等）。
    InvalidRequest,
    /// handler 执行失败。
    HandlerFailed,
    /// 内部错误（I/O、未分类错误）。
    Internal,
}

impl RpcErrorCode {
    /// 稳定的 wire 字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactCorrupt => "artifact-corrupt",
            Self::UnknownOperation => "unknown-operation",
            Self::HandlerNotRegistered => "handler-not-registered",
            Self::DuplicateRegistration => "duplicate-registration",
            Self::InvalidRequest => "invalid-request",
            Self::HandlerFailed => "handler-failed",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for RpcErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 携带稳定错误码的 RPC 错误。
///
/// 直接作为 wire 错误体序列化为 `{"code": "...", "message": "..."}`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpcError {
    code: RpcErrorCode,
    message: String,
}

impl RpcError {
    /// 用给定错误码和消息构造。
    pub fn new(code: RpcErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// 返回错误码。
    pub fn code(&self) -> RpcErrorCode {
        self.code
    }

    /// 返回错误消息。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 把 artifact 层错误归类到 RPC 错误码。
    ///
    /// 解析 / 版本 / identity 类错误归 `ArtifactCorrupt`，I/O 与未分类错误归
    /// `Internal`——读文件失败不是 artifact 损坏。
    pub fn from_artifact(err: HermesError) -> Self {
        let code = match err.kind() {
            ErrorKind::Json | ErrorKind::Syntax | ErrorKind::Validation => RpcErrorCode::ArtifactCorrupt,
            ErrorKind::Io | ErrorKind::Custom => RpcErrorCode::Internal,
        };
        Self::new(code, err.message().to_string())
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

impl From<HermesError> for RpcError {
    fn from(err: HermesError) -> Self {
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
