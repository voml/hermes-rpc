//! Hermes RPC core — VOS ServiceContract 消费端骨架。
//!
//! 本 crate 只定义类型、约束校验和 Hermes 消费接口。它**不再**自建 parser/AST：
//! 旧的 `vos-ast` / `vos-for-rust` / `vos-openapi` / `vos-cli` 已冻结并移出 workspace。
//! Hermes 的主链是**消费 `vos-language` 产出的 ServiceContract artifact**。
//!
//! TODO(hermes): 等 `vos` crate 导出正式的 `ServiceContract` 类型后，
//! 用 `use vos::ServiceContract` 替换本 crate 内的本地占位 trait。

#![deny(missing_docs)]

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
        AddHandler, EchoHandler, HermesCanonicalType, HermesCanonicalTypeArgument,
        HermesDispatcher, HermesDispatchRequest, HermesDispatchResponse,
        HermesOperationArtifact, HermesOperationHandler, HermesOperationKind,
        HermesOperationParameter, HermesOperationProjection, HermesServiceArtifact,
        HermesServiceLoader, HermesServiceRegistry, HermesSyntaxSliceContract,
        OPERATION_CONTRACT_FORMAT_VERSION, OPERATION_ID_PREFIX,
        OPERATION_PROJECTION_STAGE,
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

/// `Object` 的有序列表别名。
pub type List = voml_collection::List<Object>;
/// `String → Object` 的字典别名。
pub type Dict = voml_collection::Dict<Object>;

// ---------------------------------------------------------------------------
// 本地错误类型（替代已废弃的 `diagnostic-quick`）
// ---------------------------------------------------------------------------

pub mod error {
    //! Hermes 内部统一错误类型。设计为薄封装，等 `vos` crate 暴露
    //! 正式错误模型后直接切换。

    use std::fmt;

    /// Hermes 统一错误值，携带分类与消息。
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

        /// 与 Hermes 支持的版本做基本校验 + span-vs-source 边界校验。
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
            let source = &e.source_units[0].source;
            for projection in &self.services {
                validate_span(&projection.span, source, "service span")?;
                if projection.name.is_empty() {
                    return Err(HermesError::new(
                        crate::error::ErrorKind::Validation,
                        "service name cannot be empty",
                    ));
                }
                if let Some(body) = &projection.body {
                    validate_span(&body.span, source, "service body span")?;
                    if let Some(slice_text) = source.get(body.span.clone()) {
                        if slice_text != body.text.as_str() {
                            return Err(HermesError::new(
                                crate::error::ErrorKind::Validation,
                                "service body syntax slice does not match source",
                            ));
                        }
                    } else {
                        return Err(HermesError::new(
                            crate::error::ErrorKind::Validation,
                            "service body span out of bounds",
                        ));
                    }
                }
            }
            Ok(())
        }
    }

    /// 公共 span 校验函数（同时服务于 Service 和将来的 Operation 校验）。
    /// 检查：start <= end、end <= source.len()、start/end 落在 char boundary。
    fn validate_span(
        span: &Range<usize>,
        source: &str,
        label: &str,
    ) -> QResult<()> {
        if span.start > span.end
            || span.end > source.len()
            || !source.is_char_boundary(span.start)
            || !source.is_char_boundary(span.end)
        {
            return Err(HermesError::new(
                crate::error::ErrorKind::Validation,
                format!("{label} out of bounds"),
            ));
        }
        Ok(())
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

    // -----------------------------------------------------------------------
    // Loader / Registry —— 同时支持旧单服务 artifact 和新 projection artifact
    // -----------------------------------------------------------------------

    /// 从 artifact 加载服务描述的 trait。
    ///
    /// 默认实现同时覆盖三种输入：
    /// - `load` / `load_file` —— 解析旧的单服务手写 artifact（向后兼容）
    /// - `load_artifact` / `load_artifact_file` —— 解析 `vos-language` 产出的
    ///   `ServiceProjectionArtifact`（真实消费主链）
    /// - `load_operation_artifact` / `load_operation_artifact_file` —— 解析
    ///   `vos-language` 产出的 `OperationProjectionArtifact`
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

        /// 解析 `vos-language` 产出的 operation projection artifact。
        fn load_operation_artifact(
            &self,
            source: &[u8],
        ) -> QResult<HermesOperationArtifact> {
            HermesOperationArtifact::from_slice(source)
        }

        /// 从文件加载 operation projection artifact。
        fn load_operation_artifact_file(
            &self,
            path: &std::path::Path,
        ) -> QResult<HermesOperationArtifact> {
            let bytes = std::fs::read(path)?;
            self.load_operation_artifact(&bytes)
        }
    }

    /// 注册与查找已加载服务描述的 trait。
    ///
    /// 默认实现同时支持：
    /// - `register` / `find` / `all` / `method` —— 旧单服务 contract 注册（向后兼容）
    /// - `register_artifact` / `register_projection` —— 从 service projection artifact
    ///   批量注册 Hermes 内部可识别的服务描述
    /// - `register_operation_artifact` / `register_operation` / `find_operation` ——
    ///   从 operation projection artifact 注册独立维护的 operations registry
    ///   （services 和 operations 分开存储）
    pub trait HermesServiceRegistry {
        /// 注册旧单服务 contract。
        fn register(&mut self, contract: HermesServiceContract) -> QResult<()> {
            let name = contract.service_name.clone();
            self.insert_contract(name, contract)
        }

        /// 按全限定名查找已注册的服务。
        fn find(&self, service_name: &str) -> Option<&HermesServiceContract>;

        /// 返回所有已注册服务的迭代。
        fn all(&self) -> Vec<&HermesServiceContract>;

        /// 按服务名 + 方法名查找已注册的方法。
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

        /// 从 `HermesOperationArtifact` 批量注册 operation。
        ///
        /// Operations 是顶层声明，不属于某个 service——Hermes 独立维护
        /// operations registry（key = operation_id），与 services 分开存储。
        fn register_operation_artifact(
            &mut self,
            artifact: &HermesOperationArtifact,
        ) -> QResult<()> {
            for operation in &artifact.operations {
                self.register_operation(operation)?;
            }
            Ok(())
        }

        /// 注册单个 operation projection 到独立的 operations registry。
        fn register_operation(
            &mut self,
            projection: &HermesOperationProjection,
        ) -> QResult<()> {
            self.insert_operation(projection.operation_id.clone(), projection.clone())
        }

        /// 按 operation_id 查找已注册的 operation。
        fn find_operation(&self, operation_id: &str) -> Option<&HermesOperationProjection>;

        /// 按 name 查找已注册的 operation（取第一个匹配）。
        fn find_operation_by_name(&self, name: &str) -> Option<&HermesOperationProjection>;

        /// 返回所有已注册 operation 的迭代。
        fn all_operations(&self) -> Vec<&HermesOperationProjection>;

        /// 内部钩子：由具体实现提供实际的服务存储逻辑。
        fn insert_contract(
            &mut self,
            name: String,
            contract: HermesServiceContract,
        ) -> QResult<()>;

        /// 内部钩子：由具体实现提供实际的 operation 存储逻辑。
        fn insert_operation(
            &mut self,
            operation_id: String,
            projection: HermesOperationProjection,
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
    ///
    /// 独立维护 services 和 operations 两个 map：
    /// - services: key = 全限定服务名（canonical path join）
    /// - operations: key = operation_id（"op_" + 16 hex）
    #[derive(Clone, Debug, Default)]
    pub struct BTreeServiceRegistry {
        services: std::collections::BTreeMap<String, HermesServiceContract>,
        operations: std::collections::BTreeMap<String, HermesOperationProjection>,
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

        fn insert_operation(
            &mut self,
            operation_id: String,
            projection: HermesOperationProjection,
        ) -> QResult<()> {
            if self.operations.contains_key(&operation_id) {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Custom,
                    format!(
                        "operation `{}` ({}) already registered",
                        projection.name, operation_id
                    ),
                ));
            }
            self.operations.insert(operation_id, projection);
            Ok(())
        }

        fn find(&self, service_name: &str) -> Option<&HermesServiceContract> {
            self.services.get(service_name)
        }

        fn all(&self) -> Vec<&HermesServiceContract> {
            self.services.values().collect()
        }

        fn find_operation(
            &self,
            operation_id: &str,
        ) -> Option<&HermesOperationProjection> {
            self.operations.get(operation_id)
        }

        fn find_operation_by_name(
            &self,
            name: &str,
        ) -> Option<&HermesOperationProjection> {
            self.operations.values().find(|op| op.name == name)
        }

        fn all_operations(&self) -> Vec<&HermesOperationProjection> {
            self.operations.values().collect()
        }
    }

    // -----------------------------------------------------------------------
    // HermesOperationArtifact —— 消费 vos-language OperationProjectionArtifact
    // -----------------------------------------------------------------------
    //
    // 以下类型全部与 `vos_types::projection::*` 保持 bit-exact 的 serde 对齐
    // （camelCase 字段名 + kebab-case 枚举变体），Hermes 可以直接解析上游
    // JSON artifact 而无需其编译依赖。

    /// Operation projection artifact 支持的 contract format version。
    pub const OPERATION_CONTRACT_FORMAT_VERSION: &str = "vos-operation-contract-v0";
    /// Operation projection artifact 支持的 projection stage。
    pub const OPERATION_PROJECTION_STAGE: &str = "operation-syntax-projection";
    /// Operation identity 前缀，与上游 `OPERATION_ID_PREFIX` 对齐。
    pub const OPERATION_ID_PREFIX: &str = "op_";

    /// Hermes 从 `vos-language` 解析的 operation projection artifact。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesOperationArtifact {
        /// Artifact envelope 和来源元信息。
        pub envelope: HermesContractEnvelope,
        /// Source order 中投影出的所有 operation 声明。
        pub operations: Vec<HermesOperationProjection>,
    }

    /// Hermes 端投影出的 operation 声明，与上游 `OperationProjection` 对齐。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesOperationProjection {
        /// Operation 声明 kind。
        pub kind: HermesOperationKind,
        /// Operation 名称。
        pub name: String,
        /// Canonical signature key（durable identity 输入）。
        pub identity_key: String,
        /// Opaque durable operation identity（前缀 op_ + 16 hex）。
        pub operation_id: String,
        /// Parameters projected from Oak's signature。
        pub parameters: Vec<HermesOperationParameter>,
        /// Return type（可选，当声明有 arrow return type 时）。
        pub return_type: Option<HermesCanonicalType>,
        /// Exact signature source slice。
        pub signature: Option<HermesSyntaxSliceContract>,
        /// Exact body source slice。
        pub body: Option<HermesSyntaxSliceContract>,
        /// Full declaration span。
        #[serde(
            serialize_with = "serialize_span",
            deserialize_with = "deserialize_span"
        )]
        pub span: Range<usize>,
    }

    /// Hermes 端 operation kind，与上游 `OperationKind` 对齐。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum HermesOperationKind {
        /// 只读查询类 operation。
        Query,
        /// 用户自定义函数。
        Udf,
        /// 微型副作用 operation。
        Micro,
        /// 宏级副作用 operation。
        Macro,
    }

    /// Hermes 端单个 operation parameter，与上游 `OperationParameter` 对齐。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesOperationParameter {
        /// Parameter name。
        pub name: String,
        /// Canonical parameter type。
        pub type_expr: HermesCanonicalType,
        /// Full parameter span。
        #[serde(
            serialize_with = "serialize_span",
            deserialize_with = "deserialize_span"
        )]
        pub span: Range<usize>,
    }

    /// Hermes 端 canonical type algebra，与上游 `CanonicalType` 对齐。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case", rename_all_fields = "camelCase")]
    pub enum HermesCanonicalType {
        /// Named builtin or user type path。
        Named(Vec<String>),
        /// Primary-key reference wrapper。
        Reference(Box<HermesCanonicalType>),
        /// Optional wrapper。
        Optional(Box<HermesCanonicalType>),
        /// List wrapper。
        List(Box<HermesCanonicalType>),
        /// Generic type with canonical arguments。
        Generic {
            /// Generic name path。
            path: Vec<String>,
            /// Generic arguments in source order。
            arguments: Vec<HermesCanonicalTypeArgument>,
        },
    }

    /// Hermes 端 canonical generic argument，与上游 `CanonicalTypeArgument` 对齐。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum HermesCanonicalTypeArgument {
        /// Nested type argument。
        Type(HermesCanonicalType),
        /// Literal argument retained as canonical source text。
        Literal(String),
    }

    /// Hermes 端 syntax slice contract，与上游 `SyntaxSliceContract` 对齐。
    ///
    /// 注意：`HermesSyntaxSlice`（service body 用）和本结构体字段完全一致，
    /// 但为了与上游 serde 精确对齐，这里独立定义。
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    pub struct HermesSyntaxSliceContract {
        /// Exact source text。
        pub text: String,
        /// Source byte span。
        #[serde(
            serialize_with = "serialize_span",
            deserialize_with = "deserialize_span"
        )]
        pub span: Range<usize>,
    }

    // --- HermesOperationArtifact 解析方法 -----------------------------------

    impl HermesOperationArtifact {
        /// 从 JSON 字符串解析并做版本+span+operation_id 校验。
        pub fn from_json(input: &str) -> QResult<Self> {
            let artifact: Self = serde_json::from_str(input)?;
            artifact.validate()?;
            Ok(artifact)
        }

        /// 从字节切片解析并做版本+span+operation_id 校验。
        pub fn from_slice(source: &[u8]) -> QResult<Self> {
            let artifact: Self = serde_json::from_slice(source)?;
            artifact.validate()?;
            Ok(artifact)
        }

        /// 序列化为 pretty-printed JSON。
        pub fn to_json(&self) -> QResult<String> {
            Ok(serde_json::to_string_pretty(self)?)
        }

        /// 版本校验 + source 形状 + span 边界 + operation_id 格式。
        fn validate(&self) -> QResult<()> {
            let e = &self.envelope;
            if e.stage != OPERATION_PROJECTION_STAGE
                || e.contract_format_version != OPERATION_CONTRACT_FORMAT_VERSION
                || e.language_version != LANGUAGE_VERSION
                || e.canonicalization_version != CANONICALIZATION_VERSION
            {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Validation,
                    format!(
                        "unsupported operation artifact version \
                         (stage={}, contractFormatVersion={}, languageVersion={}, canonicalizationVersion={})",
                        e.stage,
                        e.contract_format_version,
                        e.language_version,
                        e.canonicalization_version,
                    ),
                ));
            }
            if e.schema_fingerprint.is_some()
                || e.source_units.len() != 1
                || e.source_units[0].source_unit_id != 0
            {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Validation,
                    "invalid operation source unit or fingerprint state",
                ));
            }
            let source = &e.source_units[0].source;
            for operation in &self.operations {
                validate_span(&operation.span, source, "operation span")?;
                if operation.name.is_empty() {
                    return Err(HermesError::new(
                        crate::error::ErrorKind::Validation,
                        "operation name cannot be empty",
                    ));
                }
                if operation.identity_key.is_empty() {
                    return Err(HermesError::new(
                        crate::error::ErrorKind::Validation,
                        "operation identity key cannot be empty",
                    ));
                }
                // operation_id 格式：op_ + 16 hex
                if operation.operation_id.is_empty()
                    || !operation.operation_id.starts_with(OPERATION_ID_PREFIX)
                    || operation.operation_id.len() != OPERATION_ID_PREFIX.len() + 16
                    || !operation.operation_id[OPERATION_ID_PREFIX.len()..]
                        .chars()
                        .all(|c| c.is_ascii_hexdigit())
                {
                    return Err(HermesError::new(
                        crate::error::ErrorKind::Validation,
                        format!(
                            "operation id is malformed: {}",
                            operation.operation_id
                        ),
                    ));
                }
                for parameter in &operation.parameters {
                    validate_span(&parameter.span, source, "operation parameter span")?;
                }
                for slice in operation
                    .signature
                    .iter()
                    .chain(operation.body.iter())
                {
                    validate_span(&slice.span, source, "operation syntax slice")?;
                    if let Some(slice_text) = source.get(slice.span.clone()) {
                        if slice_text != slice.text.as_str() {
                            return Err(HermesError::new(
                                crate::error::ErrorKind::Validation,
                                "operation syntax slice does not match source",
                            ));
                        }
                    } else {
                        return Err(HermesError::new(
                            crate::error::ErrorKind::Validation,
                            "operation syntax slice span out of bounds",
                        ));
                    }
                }
            }
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Hermes RPC dispatch 原型
    // -----------------------------------------------------------------------

    /// 校验 `operation_id` 字符串形状：必须是 `"op_"` + 恰好 16 个小写/大写
    /// hex 数字。
    pub(crate) fn is_valid_operation_id(id: &str) -> bool {
        if !id.starts_with(OPERATION_ID_PREFIX) {
            return false;
        }
        let tail = &id[OPERATION_ID_PREFIX.len()..];
        tail.len() == 16 && tail.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// 用户为每个 operation 注册的处理器 trait。
    ///
    /// Hermes dispatch 层是纯路由：handler 内部的业务逻辑完全由用户用 Rust
    /// 写死，Hermes 只负责按 `operation_id` 找到合适的 handler 并调用。
    /// VOS projection 阶段还没 lowering body 到可执行代码。
    pub trait HermesOperationHandler: Send + Sync {
        /// 执行对应 operation，返回 JSON 值或错误。
        ///
        /// `projection` 提供 operation 的静态声明信息（name / kind /
        /// parameters 等），`params` 是调用方传过来的 JSON 参数对象。
        fn execute(
            &self,
            projection: &HermesOperationProjection,
            params: &serde_json::Value,
        ) -> QResult<serde_json::Value>;
    }

    // --- HermesDispatcher --------------------------------------------------

    /// Hermes RPC dispatch 核心：持有 registry + handler map，按
    /// `operation_id` 路由到用户注册的 `HermesOperationHandler`。
    #[derive(Default)]
    pub struct HermesDispatcher {
        registry: BTreeServiceRegistry,
        handlers: std::collections::BTreeMap<String, Box<dyn HermesOperationHandler>>,
    }

    impl Clone for HermesDispatcher {
        fn clone(&self) -> Self {
            // handler map 里的 opaque trait object 不能 Clone，新实例保留
            // 相同 registry 但 handler 列表为空——调用方可重新注册。
            Self {
                registry: self.registry.clone(),
                handlers: std::collections::BTreeMap::new(),
            }
        }
    }

    impl std::fmt::Debug for HermesDispatcher {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("HermesDispatcher")
                .field("registry", &self.registry)
                .field(
                    "handlers",
                    &self.handlers.keys().collect::<Vec<_>>(),
                )
                .finish()
        }
    }

    impl HermesDispatcher {
        /// 构造一个空的 dispatcher。
        pub fn new() -> Self {
            Self::default()
        }

        /// 用已有 registry 构造 dispatcher。
        pub fn with_registry(registry: BTreeServiceRegistry) -> Self {
            Self { registry, handlers: std::collections::BTreeMap::new() }
        }

        /// 返回一个共享的 `JsonServiceLoader` 实例（无状态，调用零成本）。
        pub fn loader(&self) -> JsonServiceLoader {
            JsonServiceLoader::default()
        }

        /// 只读访问内部 registry。
        pub fn registry(&self) -> &BTreeServiceRegistry {
            &self.registry
        }

        /// 从 JSON 字符串解析 `HermesServiceArtifact` 并注册到内部 registry。
        pub fn load_service_artifact(&mut self, json: &str) -> QResult<()> {
            let artifact = HermesServiceArtifact::from_json(json)?;
            self.registry.register_artifact(&artifact)
        }

        /// 从 JSON 字符串解析 `HermesOperationArtifact` 并注册到内部 registry。
        pub fn load_operation_artifact(&mut self, json: &str) -> QResult<()> {
            let artifact = HermesOperationArtifact::from_json(json)?;
            self.registry.register_operation_artifact(&artifact)
        }

        /// 为 `operation_id` 注册一个 handler。
        ///
        /// 会先校验 `operation_id` 形状（`op_` + 16 hex），再校验 registry
        /// 里确实存在这个 operation——提前暴露注册手误，防止 dispatch
        /// 才发现。
        pub fn register_handler(
            &mut self,
            operation_id: &str,
            handler: Box<dyn HermesOperationHandler>,
        ) -> QResult<()> {
            if !is_valid_operation_id(operation_id) {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Validation,
                    format!("operation_id malformed: {}", operation_id),
                ));
            }
            if self.registry.find_operation(operation_id).is_none() {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Validation,
                    format!(
                        "operation `{}` not found in registry — cannot register handler",
                        operation_id
                    ),
                ));
            }
            if self.handlers.contains_key(operation_id) {
                return Err(HermesError::new(
                    crate::error::ErrorKind::Custom,
                    format!(
                        "handler for operation `{}` already registered",
                        operation_id
                    ),
                ));
            }
            self.handlers.insert(operation_id.to_string(), handler);
            Ok(())
        }

        /// 按 `operation_id` 直接 dispatch。
        ///
        /// 找不到 handler 时返回 `ErrorKind::Custom` + `"handler not registered"`；
        /// registry 里找不到这个 operation 时返回
        /// `ErrorKind::Custom` + `"operation not found"`。
        pub fn dispatch(
            &self,
            operation_id: &str,
            params: &serde_json::Value,
        ) -> QResult<serde_json::Value> {
            let projection = self.registry.find_operation(operation_id).ok_or_else(|| {
                HermesError::new(
                    crate::error::ErrorKind::Custom,
                    format!("operation not found: {}", operation_id),
                )
            })?;
            let handler = self.handlers.get(operation_id).ok_or_else(|| {
                HermesError::new(
                    crate::error::ErrorKind::Custom,
                    format!("handler not registered for operation {}", operation_id),
                )
            })?;
            handler.execute(projection, params)
        }

        /// 按 operation 名称 dispatch。先在 registry 里按 `name` 找 operation，
        /// 拿到 `operation_id` 后走 `dispatch`。
        ///
        /// 找不到时返回 `ErrorKind::Custom` + `"operation not found by name"`。
        pub fn dispatch_by_name(
            &self,
            name: &str,
            params: &serde_json::Value,
        ) -> QResult<serde_json::Value> {
            let projection = self
                .registry
                .find_operation_by_name(name)
                .ok_or_else(|| {
                    HermesError::new(
                        crate::error::ErrorKind::Custom,
                        format!("operation not found by name: {}", name),
                    )
                })?;
            self.dispatch(&projection.operation_id, params)
        }

        /// 解析 `HermesDispatchRequest`：如果 `operation` 字段以 `op_` 开头
        /// 视为 `operation_id`，否则视为 operation 名称。
        pub fn dispatch_request(
            &self,
            request: &HermesDispatchRequest,
        ) -> HermesDispatchResponse {
            let is_id = is_valid_operation_id(&request.operation);
            let (result, operation_id) = if is_id {
                (
                    self.dispatch(&request.operation, &request.params),
                    request.operation.clone(),
                )
            } else {
                match self.registry.find_operation_by_name(&request.operation) {
                    Some(op) => (
                        self.dispatch(&op.operation_id, &request.params),
                        op.operation_id.clone(),
                    ),
                    None => (
                        Err(HermesError::new(
                            crate::error::ErrorKind::Custom,
                            format!("operation not found by name: {}", request.operation),
                        )),
                        String::new(),
                    ),
                }
            };
            match result {
                Ok(value) => HermesDispatchResponse {
                    result: Some(value),
                    error: None,
                    operation_id,
                },
                Err(err) => HermesDispatchResponse {
                    result: None,
                    error: Some(err.message().to_string()),
                    operation_id,
                },
            }
        }
    }

    // --- Request / Response ------------------------------------------------

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

    // --- 内建 handler ------------------------------------------------------

    /// 把 `params` 原样返回的测试 handler，用于 fixture 的 `query A(): U`。
    #[derive(Clone, Copy, Debug, Default)]
    pub struct EchoHandler;

    impl HermesOperationHandler for EchoHandler {
        fn execute(
            &self,
            _projection: &HermesOperationProjection,
            params: &serde_json::Value,
        ) -> QResult<serde_json::Value> {
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
        fn execute(
            &self,
            _projection: &HermesOperationProjection,
            params: &serde_json::Value,
        ) -> QResult<serde_json::Value> {
            let a = params
                .get("a")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| {
                    HermesError::new(
                        crate::error::ErrorKind::Validation,
                        "missing or non-integer param `a`",
                    )
                })?;
            let b = params
                .get("b")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| {
                    HermesError::new(
                        crate::error::ErrorKind::Validation,
                        "missing or non-integer param `b`",
                    )
                })?;
            Ok(serde_json::Value::Number(serde_json::Number::from(a + b)))
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
mod schema;
pub mod validator;
