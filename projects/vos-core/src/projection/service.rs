//! service projection artifact 的本地镜像。
//!
//! 字段命名与 span 序列化必须与上游
//! `vos_types::projection::ServiceProjectionArtifact` **完全一致**。

use core::range::Range;

use serde::{Deserialize, Serialize};

use super::{
    CANONICALIZATION_VERSION, LANGUAGE_VERSION, SERVICE_CONTRACT_FORMAT_VERSION, SUPPORTED_STAGE, span::validate_span,
};
use crate::error::{ErrorKind, HermesError, QResult};

/// 服务身份前缀，与上游 `vos-types` 的 `SERVICE_ID_PREFIX` 一致。
pub const SERVICE_ID_PREFIX: &str = "svc_";

/// 从 canonical path 与 name 派生稳定服务身份。
///
/// 输出为 `svc_` + `sha256("path/name")` 前 8 字节的小写 hex，与上游
/// `vos_types::projection::compute_service_id` 完全一致。
pub fn compute_service_id(canonical_path: &[String], name: &str) -> String {
    use sha2::{Digest, Sha256};

    let path_key = canonical_path.join("/");
    let mut hasher = Sha256::new();
    hasher.update(path_key.as_bytes());
    hasher.update(b"/");
    hasher.update(name.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(SERVICE_ID_PREFIX.len() + 16);
    hex.push_str(SERVICE_ID_PREFIX);
    for byte in digest.iter().take(8) {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// 校验 `service_id` 形状：`"svc_"` + 恰好 16 个 hex 数字。
fn is_valid_service_id(id: &str) -> bool {
    match id.strip_prefix(SERVICE_ID_PREFIX) {
        Some(tail) => tail.len() == 16 && tail.chars().all(|c| c.is_ascii_hexdigit()),
        None => false,
    }
}

/// Hermes 从 `vos-language` 解析的 service projection artifact。
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
    /// 由 canonical path 与 name 派生的稳定服务身份（`svc_` + 16 hex）。
    pub service_id: String,
    /// 可选的 body 源码切片（当前 syntax-projection 阶段不做 body lowering）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<HermesSyntaxSlice>,
    /// 整个服务声明在源文本中的 span。
    #[serde(serialize_with = "super::span::serialize_span", deserialize_with = "super::span::deserialize_span")]
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
    #[serde(serialize_with = "super::span::serialize_span", deserialize_with = "super::span::deserialize_span")]
    pub span: Range<usize>,
}

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

    /// 与支持的版本做基本校验 + span-vs-source 边界校验 + service id 一致性校验。
    fn validate(&self) -> QResult<()> {
        let e = &self.envelope;
        if e.stage != SUPPORTED_STAGE
            || e.contract_format_version != SERVICE_CONTRACT_FORMAT_VERSION
            || e.language_version != LANGUAGE_VERSION
            || e.canonicalization_version != CANONICALIZATION_VERSION
        {
            return Err(HermesError::new(
                ErrorKind::Validation,
                format!(
                    "unsupported service artifact version \
                     (stage={}, contractFormatVersion={}, languageVersion={}, canonicalizationVersion={})",
                    e.stage, e.contract_format_version, e.language_version, e.canonicalization_version,
                ),
            ));
        }
        if e.source_units.len() != 1 || e.source_units[0].source_unit_id != 0 {
            return Err(HermesError::new(ErrorKind::Validation, "invalid source_units state"));
        }
        let source = &e.source_units[0].source;
        for projection in &self.services {
            validate_span(&projection.span, source, "service span")?;
            if projection.name.is_empty() {
                return Err(HermesError::new(ErrorKind::Validation, "service name cannot be empty"));
            }
            if !is_valid_service_id(&projection.service_id) {
                return Err(HermesError::new(
                    ErrorKind::Validation,
                    format!("service id is malformed: {}", projection.service_id),
                ));
            }
            let expected = compute_service_id(&projection.canonical_path, &projection.name);
            if projection.service_id != expected {
                return Err(HermesError::new(
                    ErrorKind::Validation,
                    format!(
                        "service id does not match canonical path and name: \
                         expected {expected}, got {}",
                        projection.service_id
                    ),
                ));
            }
            if let Some(body) = &projection.body {
                validate_span(&body.span, source, "service body span")?;
                if let Some(slice_text) = source.get(body.span.clone()) {
                    if slice_text != body.text.as_str() {
                        return Err(HermesError::new(ErrorKind::Validation, "service body syntax slice does not match source"));
                    }
                }
                else {
                    return Err(HermesError::new(ErrorKind::Validation, "service body span out of bounds"));
                }
            }
        }
        Ok(())
    }
}
