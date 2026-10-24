//! Hermes 合同 envelope 的加载与校验。
//!
//! 合同类型、identity、fingerprint 与 canonical type 全部来自上游 `vos-types`
//! （`vos-language`），本 crate 不定义任何语义副本。Hermes 只消费
//! `vos/hermes.contract/v0`：其他产品格式、未知版本、未知字段与指纹不匹配都在
//! 加载期拒绝，并保留上游诊断码（`ART010` / `ART020` / `ART021` 与 payload 的
//! `OPC` / `SVC` 码）。
//!
//! 合同不携带 VOS body，Hermes 也不会重新解析 VOS 源文本补齐信息。

use std::path::Path;

use vos_types::ContractArtifact;

use crate::error::{RpcError, RpcErrorCode, RpcResult};

/// Hermes 接受的唯一合同格式。
pub const HERMES_CONTRACT_FORMAT: &str = "vos/hermes.contract/v0";

/// 解析并校验一份 Hermes 合同 artifact。
///
/// 先按 serde 解码（未知字段与未知格式版本 → `ART010`），再确认产品格式是
/// Hermes（其他格式仍按 `ART010` 拒绝），最后执行上游 payload 校验（指纹
/// `ART020`、producer `ART021`、合同内部 `OPC` / `SVC` 码）。
pub fn parse_contract_artifact(json: &str) -> RpcResult<ContractArtifact> {
    let artifact: ContractArtifact = serde_json::from_str(json)
        .map_err(|error| RpcError::new(RpcErrorCode::ArtifactCorrupt, error.to_string()).with_diagnostic_code("ART010"))?;
    if artifact.version.as_str() != HERMES_CONTRACT_FORMAT {
        return Err(RpcError::new(
            RpcErrorCode::ArtifactCorrupt,
            format!("Hermes accepts only `{HERMES_CONTRACT_FORMAT}`, found `{}`", artifact.version.as_str()),
        )
        .with_diagnostic_code("ART010"));
    }
    artifact.validate().map_err(RpcError::from_artifact)?;
    Ok(artifact)
}

/// 从文件读取一份 Hermes 合同 artifact。
pub fn read_contract_artifact(path: &Path) -> RpcResult<ContractArtifact> {
    let json = std::fs::read_to_string(path)?;
    parse_contract_artifact(&json)
}

/// 把合同 payload 重新封装成 Hermes artifact。
///
/// 只用于已经按上游规则校验过的 payload：fingerprint 由上游
/// `ContractArtifact::seal` 重新计算，Hermes 不参与指纹推导。
pub fn seal_hermes_artifact(artifact: &ContractArtifact) -> ContractArtifact {
    ContractArtifact::seal(vos_types::ContractFormat::Hermes, artifact.producer.clone(), artifact.payload.clone())
}
