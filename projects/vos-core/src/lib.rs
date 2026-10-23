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
    hermes::{
        HermesServiceArtifact, HermesServiceLoader, HermesServiceRegistry,
    },
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
    //! Hermes 消费 VOS ServiceProjectionArtifact 的本地镜像与加载骨架。
    //!
    //! 本模块**不编译依赖** `vos` crate：所有 `Hermes*` 结构体与上游
    //! `vos-types::projection::ServiceProjectionArtifact` 保持 bit-exact 的
    //! serde 对齐（camelCase 字段名 + `SpanWire` 序列化），因此 Hermes 可以
    //! 直接解析 `vos-language` 产出的 JSON artifact 而无需其编译依赖。
    //! 上游 `oak-vos` 迁到 stable 后，可切换为 `use vos::ServiceProjectionArtifact`。

    use core::range::Range;

    use serde::{Deserialize, Serialize};

    use crate::error::{HermesError, QResult};

    // -----------------------------------------------------------------------
    // 与 vos-language `ServiceProjectionArtifact` 完全 serde 对齐的本地镜像
    // -----------------------------------------------------------------------

    /// Hermes 从 `vos-language` 解析的 service projection artifact。
    ///
    /// 字段命名和 span 序列化必须与
    /// `vos_types::projection::ServiceProjectionArtifact` **完全一致**
    /// （camelCase + `{start, end}` 形状），否则无法解析上游 JSON。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesServiceArtifact {
        /// Artifact envelope 和来源元信息。
        pub envelope: HermesContractEnvelope,
        /// Source order 中投影出的所有服务。
        pub services: Vec<HermesServiceProjection>,
    }

    /// Artifact 版本与来源 envelope，与上游 `ContractEnvelope` 对齐。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesContractEnvelope {
        /// Artifact 处理阶段（例如 `"service-syntax-projection"`）。
        pub stage: String,
        /// Artifact 格式版本。
        pub contract_format_version: String,
        /// VOS 语言合同版本。
        pub language_version: String,
        /// Canonicalization 规则版本。
        pub canonicalization_version: String,
        /// 可选 schema fingerprint。
        pub schema_fingerprint: Option<String>,
        /// Artifact 包含的 source units。
        pub source_units: Vec<HermesSourceUnit>,
    }

    /// Artifact 携带的单个 source unit。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesSourceUnit {
        /// Artifact 内稳定的 source unit id。
        pub source_unit_id: u32,
        /// 精确的源文本。
        pub source: String,
    }

    /// Hermes 端投影出的服务声明，与上游 `ServiceProjection` 对齐。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesServiceProjection {
        /// 服务名（原始声明名）。
        pub name: String,
        /// Canonical namespace 限定的路径（source order）。
        pub canonical_path: Vec<String>,
        /// 可选的 body 源码切片（当前 syntax-projection 阶段不做 body lowering）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub body: Option<HermesSyntaxSlice>,
        /// 整个服务声明在源文本中的 span。
        #[serde(
            serialize_with = "serialize_span",
            deserialize_with = "deserialize_span"
        )]
        pub span: Range<usize>,
    }

    /// 语法切片（body / signature / source）。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesSyntaxSlice {
        /// 精确的源文本。
        pub text: String,
        /// 在源中的字节 span。
        #[serde(
            serialize_with = "serialize_span",
            deserialize_with = "deserialize_span"
        )]
        pub span: Range<usize>,
    }

    /// Artifact 支持的 contract format version。
    pub const SERVICE_CONTRACT_FORMAT_VERSION: &str = "vos-service-contract-v0";
    /// Artifact 支持的 language version。
    pub const LANGUAGE_VERSION: &str = "vos-language-v0";
    /// Artifact 支持的 canonicalization version。
    pub const CANONICALIZATION_VERSION: &str = "vos-canonical-v0";
    /// Hermes 当前能消费的 projection stage。
    pub const SUPPORTED_STAGE: &str = "service-syntax-projection";

    // --- HermesServiceArtifact 解析方法 -------------------------------------

    impl HermesServiceArtifact {
        /// 从 JSON 字符串解析并做版本校验。
        pub fn from_json(input: &str) -> QResult<Self> {
            let artifact: Self = serde_json::from_str(input)?;
            artifact.validate()?;
            Ok(artifact)
        }

        /// 从字节切片解析并做版本校验。
        pub fn from_slice(source: &[u8]) -> QResult<Self> {
            let artifact: Self = serde_json::from_slice(source)?;
            artifact.validate()?;
            Ok(artifact)
        }

        /// 序列化为 pretty-printed JSON。
        pub fn to_json(&self) -> QResult<String> {
            Ok(serde_json::to_string_pretty(self)?)
        }

        /// 与 Hermes 支持的版本做基本校验。
        fn validate(&self) -> QResult<()> {
            let e = &self.envelope;
            if e.stage != SUPPORTED_STAGE
                || e.contract_format_version != SERVICE_CONTRACT_FORMAT_VERSION
                || e.language_version != LANGUAGE_VERSION
                || e.canonicalization_version != CANONICALIZATION_VERSION
            {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Validation,
                    format!(
                        "unsupported service artifact version \
                         (stage={}, contractFormatVersion={}, languageVersion={}, canonicalizationVersion={})",
                        e.stage,
                        e.contract_format_version,
                        e.language_version,
                        e.canonicalization_version,
                    ),
                ));
            }
            if e.source_units.len() != 1 || e.source_units[0].source_unit_id != 0 {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Validation,
                    "invalid source_units state",
                ));
            }
            Ok(())
        }
    }

    // --- Span 序列化 / 反序列化（与 vos-types 1:1 对齐） --------------------

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct SpanWire {
        start: usize,
        end: usize,
    }

    fn serialize_span<S: serde::Serializer>(
        span: &Range<usize>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        SpanWire { start: span.start, end: span.end }.serialize(serializer)
    }

    fn deserialize_span<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Range<usize>, D::Error> {
        let span = SpanWire::deserialize(deserializer)?;
        if span.start > span.end {
            return Err(serde::de::Error::custom("span start is after span end"));
        }
        Ok((span.start..span.end).into())
    }

    // -----------------------------------------------------------------------
    // 旧占位 ServiceContract 结构 —— 向后兼容（单服务手写 artifact）
    // -----------------------------------------------------------------------

    /// Hermes 端占位的服务描述（单服务手写 artifact 场景）。
    ///
    /// 真实消费主链是 `HermesServiceArtifact`（与上游 `ServiceProjectionArtifact`
    /// 对齐）。本结构体仅用于旧 mock 测试与手工构造的单服务 JSON。
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

    // -----------------------------------------------------------------------
    // Loader / Registry —— 同时支持旧单服务 artifact 和新 projection artifact
    // -----------------------------------------------------------------------

    /// 从 artifact 加载服务描述的 trait。
    ///
    /// 默认实现同时覆盖两种输入：
    /// - `load` / `load_file` —— 解析旧的单服务手写 artifact（向后兼容）
    /// - `load_artifact` / `load_artifact_file` —— 解析 `vos-language` 产出的
    ///   `ServiceProjectionArtifact`（真实消费主链）
    pub trait HermesServiceLoader {
        /// 解析旧的单服务手写 artifact（JSON 形状是 `HermesServiceContract`）。
        fn load(&self, source: &[u8]) -> QResult<HermesServiceContract> {
            let contract: HermesServiceContract = serde_json::from_slice(source)?;
            Ok(contract)
        }

        /// 从文件加载旧的单服务 artifact。
        fn load_file(&self, path: &std::path::Path) -> QResult<HermesServiceContract> {
            let bytes = std::fs::read(path)?;
            self.load(&bytes)
        }

        /// 解析 `vos-language` 产出的 service projection artifact。
        fn load_artifact(&self, source: &[u8]) -> QResult<HermesServiceArtifact> {
            HermesServiceArtifact::from_slice(source)
        }

        /// 从文件加载 service projection artifact。
        fn load_artifact_file(
            &self,
            path: &std::path::Path,
        ) -> QResult<HermesServiceArtifact> {
            let bytes = std::fs::read(path)?;
            self.load_artifact(&bytes)
        }
    }

    /// 注册与查找已加载服务描述的 trait。
    ///
    /// 默认实现同时支持：
    /// - `register` / `find` / `all` / `method` —— 旧单服务 contract 注册（向后兼容）
    /// - `register_artifact` / `register_projection` —— 从 projection artifact
    ///   批量注册 Hermes 内部可识别的服务描述
    pub trait HermesServiceRegistry {
        /// 注册旧单服务 contract。
        fn register(&mut self, contract: HermesServiceContract) -> QResult<()> {
            let name = contract.service_name.clone();
            self.insert_contract(name, contract)
        }

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

        /// 从 `HermesServiceArtifact` 批量注册内部服务描述。
        fn register_artifact(&mut self, artifact: &HermesServiceArtifact) -> QResult<()> {
            for projection in &artifact.services {
                self.register_projection(projection)?;
            }
            Ok(())
        }

        /// 把单个 `HermesServiceProjection` 注册为内部可识别的服务描述。
        ///
        /// 当前 projection 阶段（`service-syntax-projection`）尚未携带 methods，
        /// 因此转换得到的 `HermesServiceContract.methods` 为空。将来更低
        /// 阶段的 projection 可以覆盖这个默认行为。
        fn register_projection(
            &mut self,
            projection: &HermesServiceProjection,
        ) -> QResult<()> {
            let service_name = if !projection.canonical_path.is_empty() {
                projection.canonical_path.join(".")
            } else {
                projection.name.clone()
            };
            let contract = HermesServiceContract {
                service_name,
                methods: vec![],
                revision: None,
            };
            self.register(contract)
        }

        /// 内部钩子：由具体实现（如 `BTreeServiceRegistry`）提供实际的
        /// 存储逻辑，避免默认实现和 concrete impl 互相循环调用 `register`。
        fn insert_contract(
            &mut self,
            name: String,
            contract: HermesServiceContract,
        ) -> QResult<()>;
    }

    // -----------------------------------------------------------------------
    // 内建 JSON Loader
    // -----------------------------------------------------------------------

    /// 用 `serde_json` 解析 artifact 的默认 loader。
    #[derive(Clone, Debug, Default)]
    pub struct JsonServiceLoader;

    impl HermesServiceLoader for JsonServiceLoader {}

    // -----------------------------------------------------------------------
    // 内建 BTreeMap Registry
    // -----------------------------------------------------------------------

    /// 基于 `BTreeMap` 的简单 registry，适合单机 Hermes 进程。
    #[derive(Clone, Debug, Default)]
    pub struct BTreeServiceRegistry {
        services: std::collections::BTreeMap<String, HermesServiceContract>,
    }

    impl BTreeServiceRegistry {
        /// 按全限定名（canonical path join）或原始名查找 projection artifact 中的服务。
        pub fn find_by_name_or_canonical(
            &self,
            projection: &HermesServiceProjection,
        ) -> Option<&HermesServiceContract> {
            let canonical = if !projection.canonical_path.is_empty() {
                projection.canonical_path.join(".")
            } else {
                projection.name.clone()
            };
            self.find(&canonical).or_else(|| self.find(&projection.name))
        }
    }

    impl HermesServiceRegistry for BTreeServiceRegistry {
        fn insert_contract(
            &mut self,
            name: String,
            contract: HermesServiceContract,
        ) -> QResult<()> {
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
