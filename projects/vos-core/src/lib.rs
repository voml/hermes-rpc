//! Hermes 仓的 VOS 语言层宿主。
//!
//! 本 crate 承载 VOS 语言标准在仓内的两部分能力：
//!
//! - **语言层**：`Project` / `Endpoint` / constraint / `validator` / `faker` /
//!   `pretty_print` 等 schema 建模、约束与渲染。
//! - **artifact 合同镜像**：与 `vos-language` 的 `vos-types` 保持 serde 对齐的
//!   service / operation projection 结构，以及 `svc_` / `op_` 稳定 identity 派生。
//!
//! RPC 领域逻辑（loader / registry / router / dispatch / handler 注册）在
//! `hermes-runtime`。本 crate 不再承载、也不再新增 RPC 领域能力。
//!
//! 本 crate **不自建 VOS parser/AST**：旧的 `vos-ast` / `vos-for-rust` /
//! `vos-openapi` / `vos-cli` 已冻结并移出 workspace。

#![deny(missing_docs)]

pub use self::{
    constraint::{
        SharedConstraint,
        decimal_constraint::DecimalConstraint,
        dict_constraint::DictConstraint,
        integer_constraint::{IntegerConstraint, IntegerKind},
        list_constraint::ListConstraint,
        string_constraint::StringConstraint,
    },
    error::{HermesError, QError, QResult},
    projection::{
        HermesCanonicalType, HermesCanonicalTypeArgument, HermesContractEnvelope, HermesOperationArtifact, HermesOperationKind,
        HermesOperationParameter, HermesOperationProjection, HermesServiceArtifact, HermesServiceProjection, HermesSourceUnit,
        HermesSyntaxSlice, HermesSyntaxSliceContract, OPERATION_CONTRACT_FORMAT_VERSION, OPERATION_ID_PREFIX,
        OPERATION_PROJECTION_STAGE, SERVICE_ID_PREFIX, compute_service_id,
    },
    schema::{
        Project, ProjectKind, Schema,
        authors::ProjectAuthor,
        document::{Document, DocumentKind},
        endpoint::Endpoint,
        environment::Environment,
        license::License,
        objects::Object,
    },
};

use std::fmt;
pub use voml_collection::Text;

/// `Object` 的有序列表别名。
pub type List = voml_collection::List<Object>;
/// `String → Object` 的字典别名。
pub type Dict = voml_collection::Dict<Object>;

// ---------------------------------------------------------------------------
// 本地错误类型（替代已废弃的 `diagnostic-quick`）
// ---------------------------------------------------------------------------

pub mod error {
    //! VOS 语言层与 artifact 校验共用的薄错误类型。

    use std::fmt;

    /// 统一错误值，携带分类与消息。
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct HermesError {
        kind: ErrorKind,
        message: String,
    }

    /// 错误分类枚举，供上层按类型分派处理。
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum ErrorKind {
        /// 语法层面的错误（例如 artifact 形状不符合预期）。
        Syntax,
        /// 校验失败（semver/url/email/span 边界等）。
        Validation,
        /// I/O 错误（读文件等）。
        Io,
        /// JSON 解析错误。
        Json,
        /// 其他自定义错误。
        Custom,
    }

    impl HermesError {
        /// 用给定分类和消息构造一个新的 `HermesError`。
        pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
            Self { kind, message: message.into() }
        }

        /// 返回错误分类。
        pub fn kind(&self) -> ErrorKind {
            self.kind
        }

        /// 返回错误消息切片。
        pub fn message(&self) -> &str {
            &self.message
        }
    }

    impl fmt::Display for HermesError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{:?}: {}", self.kind, self.message)
        }
    }

    impl std::error::Error for HermesError {}

    impl From<Box<dyn std::error::Error + Send + Sync>> for HermesError {
        fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
            Self { kind: ErrorKind::Custom, message: e.to_string() }
        }
    }

    impl From<std::io::Error> for HermesError {
        fn from(e: std::io::Error) -> Self {
            Self { kind: ErrorKind::Io, message: e.to_string() }
        }
    }

    impl From<serde_json::Error> for HermesError {
        fn from(e: serde_json::Error) -> Self {
            Self { kind: ErrorKind::Json, message: e.to_string() }
        }
    }

    impl From<semver::Error> for HermesError {
        fn from(e: semver::Error) -> Self {
            Self { kind: ErrorKind::Validation, message: e.to_string() }
        }
    }

    impl From<url::ParseError> for HermesError {
        fn from(e: url::ParseError) -> Self {
            Self { kind: ErrorKind::Validation, message: e.to_string() }
        }
    }

    impl From<email_address::Error> for HermesError {
        fn from(e: email_address::Error) -> Self {
            Self { kind: ErrorKind::Validation, message: e.to_string() }
        }
    }

    impl From<num_bigint::ParseBigIntError> for HermesError {
        fn from(e: num_bigint::ParseBigIntError) -> Self {
            Self { kind: ErrorKind::Validation, message: e.to_string() }
        }
    }

    impl From<rust_decimal::Error> for HermesError {
        fn from(e: rust_decimal::Error) -> Self {
            Self { kind: ErrorKind::Validation, message: e.to_string() }
        }
    }

    impl From<std::num::ParseIntError> for HermesError {
        fn from(e: std::num::ParseIntError) -> Self {
            Self { kind: ErrorKind::Validation, message: e.to_string() }
        }
    }

    impl From<std::num::ParseFloatError> for HermesError {
        fn from(e: std::num::ParseFloatError) -> Self {
            Self { kind: ErrorKind::Validation, message: e.to_string() }
        }
    }

    /// 零值快捷错误构造器（替代旧 `diagnostic-quick::QError`）。
    pub struct QError;

    impl QError {
        /// 快速构造 `ErrorKind::Syntax` 错误。
        pub fn syntax_error(msg: impl Into<String>) -> HermesError {
            HermesError::new(ErrorKind::Syntax, msg)
        }
        /// 快速构造 `ErrorKind::Validation` 错误。
        pub fn validation_error(msg: impl Into<String>) -> HermesError {
            HermesError::new(ErrorKind::Validation, msg)
        }
        /// 快速构造 `ErrorKind::Custom` 错误。
        pub fn custom(msg: impl Into<String>) -> HermesError {
            HermesError::new(ErrorKind::Custom, msg)
        }
    }

    /// 零值 Result 别名（替代旧 `diagnostic-quick::QResult`）。
    pub type QResult<T = ()> = Result<T, HermesError>;
}

// ---------------------------------------------------------------------------
// Validation<T> —— 带诊断的成功/失败结果
// ---------------------------------------------------------------------------

/// 带诊断信息的成功/失败结果，比 `Result<T, HermesError>` 多携带一条 diagnostics 列表。
#[derive(Clone, Debug)]
pub enum Validation<T> {
    /// 成功分支，附带可选的 diagnostics（warn / info 等）。
    Success {
        /// 成功时携带的值。
        value: T,
        /// 诊断信息列表（warn / info 级别）。
        diagnostics: Vec<HermesError>,
    },
    /// 失败分支，携带全部诊断错误。
    Failure {
        /// 所有失败时的诊断错误。
        diagnostics: Vec<HermesError>,
    },
}

impl<T> Validation<T> {
    /// 用 `value` 构造一个不带 diagnostics 的成功结果。
    pub fn ok(value: T) -> Self {
        Self::Success { value, diagnostics: vec![] }
    }

    /// 用单个错误构造一个失败结果。
    pub fn fail(err: impl Into<HermesError>) -> Self {
        Self::Failure { diagnostics: vec![err.into()] }
    }

    /// 判断是否为成功分支。
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// 成功分支返回 `Some(value)`，失败分支返回 `None`。
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Success { value, .. } => Some(value),
            Self::Failure { .. } => None,
        }
    }

    /// 映射成功分支的值，失败分支原样透传。
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Validation<U> {
        match self {
            Self::Success { value, diagnostics } => Validation::Success { value: f(value), diagnostics },
            Self::Failure { diagnostics } => Validation::Failure { diagnostics },
        }
    }
}

impl<T: fmt::Display> fmt::Display for Validation<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success { value, .. } => write!(f, "ok({value})"),
            Self::Failure { diagnostics } => {
                write!(f, "fail(")?;
                for (i, d) in diagnostics.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{d}")?;
                }
                write!(f, ")")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Parser / Codegen trait —— 从旧栈保留的接口占位
// ---------------------------------------------------------------------------

/// 旧栈 Parser trait 占位：Hermes 不再自建 VOS parser。
pub trait Parser<S> {
    /// 解析源 `S` 得到一个带 diagnostics 的 `Project`。
    fn parse(&self, source: &S) -> Validation<Project>;
}

/// 旧栈 Codegen trait 占位：Hermes 不再自建代码生成。
pub trait Codegen {
    /// 生成产物类型。
    type Output;
    /// 从 `Project` 生成产物。
    fn generate(&self, project: &Project) -> Validation<Self::Output>;
}

// ---------------------------------------------------------------------------
// 模块声明
// ---------------------------------------------------------------------------

mod constraint;
#[cfg(feature = "faker")]
mod faker;
mod pretty_print;
pub mod projection;
mod schema;
pub mod validator;
