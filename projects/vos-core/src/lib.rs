//! Hermes RPC core — VOS ServiceContract 消费端骨架。
//!
//! 本 crate 只定义类型、约束校验和 Hermes 消费接口。它**不再**自建 parser/AST：
//! 旧的 `vos-ast` / `vos-for-rust` / `vos-openapi` / `vos-cli` 已冻结并移出 workspace。
//! Hermes 的主链是**消费 `vos-language` 产出的 ServiceContract artifact**。
//!
//! TODO(hermes): 等 `vos` crate 导出正式的 `ServiceContract` 类型后，
//! 用 `use vos::ServiceContract` 替换本 crate 内的本地占位 trait。

pub use self::{
    constraint::{
        decimal_constraint::DecimalConstraint,
        dict_constraint::DictConstraint,
        integer_constraint::{IntegerConstraint, IntegerKind},
        list_constraint::ListConstraint,
        string_constraint::StringConstraint,
        SharedConstraint,
    },
    error::{HermesError, QError, QResult},
    hermes::{HermesServiceLoader, HermesServiceRegistry},
    schema::{
        authors::ProjectAuthor,
        document::{Document, DocumentKind},
        endpoint::Endpoint,
        environment::Environment,
        license::License,
        objects::Object,
        Project, ProjectKind, Schema,
    },
};

pub use voml_collection::Text;
use std::fmt;

pub type List = voml_collection::List<Object>;
pub type Dict = voml_collection::Dict<Object>;

// ---------------------------------------------------------------------------
// 本地错误类型（替代已废弃的 `diagnostic-quick`）
// ---------------------------------------------------------------------------

pub mod error {
    //! Hermes 内部统一错误类型。设计为薄封装，等 `vos` crate 暴露
    //! 正式错误模型后直接切换。

    use std::fmt;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct HermesError {
        kind: ErrorKind,
        message: String,
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum ErrorKind {
        Syntax,
        Validation,
        Io,
        Json,
        Custom,
    }

    impl HermesError {
        pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
            Self { kind, message: message.into() }
        }

        pub fn kind(&self) -> ErrorKind {
            self.kind
        }

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
        pub fn syntax_error(msg: impl Into<String>) -> HermesError {
            HermesError::new(ErrorKind::Syntax, msg)
        }
        pub fn validation_error(msg: impl Into<String>) -> HermesError {
            HermesError::new(ErrorKind::Validation, msg)
        }
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

#[derive(Clone, Debug)]
pub enum Validation<T> {
    Success { value: T, diagnostics: Vec<HermesError> },
    Failure { diagnostics: Vec<HermesError> },
}

impl<T> Validation<T> {
    pub fn ok(value: T) -> Self {
        Self::Success { value, diagnostics: vec![] }
    }

    pub fn fail(err: impl Into<HermesError>) -> Self {
        Self::Failure { diagnostics: vec![err.into()] }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Success { value, .. } => Some(value),
            Self::Failure { .. } => None,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Validation<U> {
        match self {
            Self::Success { value, diagnostics } => {
                Validation::Success { value: f(value), diagnostics }
            }
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
// Hermes 消费接口骨架
// ---------------------------------------------------------------------------

pub mod hermes {
    //! Hermes 从 ServiceContract artifact 消费 VOS 服务描述的 trait。
    //!
    //! 这是**占位骨架**。真实的 `ServiceContract` 类型由 `vos-language`
    //! 仓库导出，Hermes 后续只做消费方，不做解析或 AST 构建。

    use serde::{Deserialize, Serialize};

    use crate::error::{HermesError, QResult};

    // --- 占位 ServiceContract 结构 ----------------------------------------

    /// Hermes 端占位的服务描述。
    ///
    /// TODO(hermes): 用 `vos::ServiceContract` 替换。字段命名保持与将来
    /// 的 VOS 正式 artifact 对齐。
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
        pub name: String,
        /// 请求体的类型名（对应 VOS schema 中的 type）。
        pub request_type: String,
        /// 响应体的类型名。
        pub response_type: String,
        #[serde(default)]
        pub docs: String,
    }

    // --- Loader & Registry ------------------------------------------------

    /// 从 artifact（通常是 JSON 文件或字节流）加载 ServiceContract。
    ///
    /// 实现可以是：
    /// - 直接 `serde_json::from_slice`（读取 vos-language 生成的 artifact）
    /// - 从 `include_bytes!` 嵌入的静态资源加载
    /// - 远端拉取（Hermes server 场景）
    pub trait HermesServiceLoader {
        fn load(&self, source: &[u8]) -> QResult<HermesServiceContract>;

        fn load_file(&self, path: &std::path::Path) -> QResult<HermesServiceContract> {
            let bytes = std::fs::read(path)?;
            self.load(&bytes)
        }
    }

    /// 注册与查找已加载的 ServiceContract。
    ///
    /// Hermes runtime 用它把服务描述和具体的 method handler 绑定。
    pub trait HermesServiceRegistry {
        fn register(&mut self, contract: HermesServiceContract) -> QResult<()>;

        fn find(&self, service_name: &str) -> Option<&HermesServiceContract>;

        fn all(&self) -> Vec<&HermesServiceContract>;

        fn method(
            &self,
            service_name: &str,
            method_name: &str,
        ) -> Option<&HermesMethodDescriptor> {
            self.find(service_name).and_then(|c| {
                c.methods.iter().find(|m| m.name == method_name)
            })
        }
    }

    // --- 内建 JSON Loader -------------------------------------------------

    /// 用 `serde_json` 解析 artifact 的默认 loader。
    #[derive(Clone, Debug, Default)]
    pub struct JsonServiceLoader;

    impl HermesServiceLoader for JsonServiceLoader {
        fn load(&self, source: &[u8]) -> QResult<HermesServiceContract> {
            let contract: HermesServiceContract = serde_json::from_slice(source)?;
            Ok(contract)
        }
    }

    // --- 内建 HashMap Registry --------------------------------------------

    /// 基于 `BTreeMap` 的简单 registry，适合单机 Hermes 进程。
    #[derive(Clone, Debug, Default)]
    pub struct BTreeServiceRegistry {
        services: std::collections::BTreeMap<String, HermesServiceContract>,
    }

    impl HermesServiceRegistry for BTreeServiceRegistry {
        fn register(&mut self, contract: HermesServiceContract) -> QResult<()> {
            let name = contract.service_name.clone();
            if self.services.contains_key(&name) {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Custom,
                    format!("service `{}` already registered", name),
                ));
            }
            self.services.insert(name, contract);
            Ok(())
        }

        fn find(&self, service_name: &str) -> Option<&HermesServiceContract> {
            self.services.get(service_name)
        }

        fn all(&self) -> Vec<&HermesServiceContract> {
            self.services.values().collect()
        }
    }
}

// ---------------------------------------------------------------------------
// Parser / Codegen trait —— 从旧栈保留的接口占位
// ---------------------------------------------------------------------------

pub trait Parser<S> {
    fn parse(&self, source: &S) -> Validation<Project>;
}

pub trait Codegen {
    type Output;
    fn generate(&self, project: &Project) -> Validation<Self::Output>;
}

// ---------------------------------------------------------------------------
// 模块声明
// ---------------------------------------------------------------------------

mod constraint;
#[cfg(feature = "faker")]
mod faker;
mod pretty_print;
mod schema;
pub mod validator;
