//! 合同 artifact 的来源抽象。
//!
//! Hermes 只消费上游产出的合同 artifact，不读 VOS 源文本。loader 负责把字节
//! 解码成 Hermes 合同，并复用 [`crate::contract::parse_contract_artifact`] 的
//! 格式、指纹与 producer 校验。

use std::path::Path;

use vos_types::ContractArtifact;

use crate::{
    contract::parse_contract_artifact,
    error::{RpcError, RpcErrorCode, RpcResult},
};

/// 把外部来源解码成 Hermes 合同 artifact。
pub trait HermesContractLoader {
    /// 从字节加载合同。`source` 必须是 Hermes artifact JSON。
    fn load_contract(&self, source: &[u8]) -> RpcResult<ContractArtifact>;

    /// 从文件加载合同。
    fn load_contract_file(&self, path: &Path) -> RpcResult<ContractArtifact> {
        let bytes = std::fs::read(path)?;
        self.load_contract(&bytes)
    }
}

/// JSON artifact loader（Hermes 的唯一内置来源形态）。
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonContractLoader;

impl HermesContractLoader for JsonContractLoader {
    fn load_contract(&self, source: &[u8]) -> RpcResult<ContractArtifact> {
        let json = std::str::from_utf8(source).map_err(|error| {
            RpcError::new(RpcErrorCode::ArtifactCorrupt, format!("contract artifact is not valid UTF-8: {error}"))
                .with_diagnostic_code("ART010")
        })?;
        parse_contract_artifact(json)
    }
}
