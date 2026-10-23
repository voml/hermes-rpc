//! operation projection artifact 的本地镜像。
//!
//! 与 `vos_types::projection::*` 保持 bit-exact 的 serde 对齐
//! （camelCase 字段名 + kebab-case 枚举变体），因此可以直接解析上游
//! JSON artifact 而无需其编译依赖。

use core::range::Range;

use serde::{Deserialize, Serialize};

use super::{CANONICALIZATION_VERSION, LANGUAGE_VERSION, span::validate_span};
use crate::error::{ErrorKind, HermesError, QResult};

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
    pub envelope: super::HermesContractEnvelope,
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
    #[serde(serialize_with = "super::span::serialize_span", deserialize_with = "super::span::deserialize_span")]
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
    #[serde(serialize_with = "super::span::serialize_span", deserialize_with = "super::span::deserialize_span")]
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
    #[serde(serialize_with = "super::span::serialize_span", deserialize_with = "super::span::deserialize_span")]
    pub span: Range<usize>,
}

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
                ErrorKind::Validation,
                format!(
                    "unsupported operation artifact version \
                     (stage={}, contractFormatVersion={}, languageVersion={}, canonicalizationVersion={})",
                    e.stage, e.contract_format_version, e.language_version, e.canonicalization_version,
                ),
            ));
        }
        if e.schema_fingerprint.is_some() || e.source_units.len() != 1 || e.source_units[0].source_unit_id != 0 {
            return Err(HermesError::new(ErrorKind::Validation, "invalid operation source unit or fingerprint state"));
        }
        let source = &e.source_units[0].source;
        for operation in &self.operations {
            validate_span(&operation.span, source, "operation span")?;
            if operation.name.is_empty() {
                return Err(HermesError::new(ErrorKind::Validation, "operation name cannot be empty"));
            }
            if operation.identity_key.is_empty() {
                return Err(HermesError::new(ErrorKind::Validation, "operation identity key cannot be empty"));
            }
            if !is_valid_operation_id(&operation.operation_id) {
                return Err(HermesError::new(
                    ErrorKind::Validation,
                    format!("operation id is malformed: {}", operation.operation_id),
                ));
            }
            for parameter in &operation.parameters {
                validate_span(&parameter.span, source, "operation parameter span")?;
            }
            for slice in operation.signature.iter().chain(operation.body.iter()) {
                validate_span(&slice.span, source, "operation syntax slice")?;
                if let Some(slice_text) = source.get(slice.span.clone()) {
                    if slice_text != slice.text.as_str() {
                        return Err(HermesError::new(ErrorKind::Validation, "operation syntax slice does not match source"));
                    }
                }
                else {
                    return Err(HermesError::new(ErrorKind::Validation, "operation syntax slice span out of bounds"));
                }
            }
        }
        Ok(())
    }
}

/// 校验 `operation_id` 形状：`"op_"` + 恰好 16 个 hex 数字。
fn is_valid_operation_id(id: &str) -> bool {
    if !id.starts_with(OPERATION_ID_PREFIX) {
        return false;
    }
    let tail = &id[OPERATION_ID_PREFIX.len()..];
    tail.len() == 16 && tail.chars().all(|c| c.is_ascii_hexdigit())
}
