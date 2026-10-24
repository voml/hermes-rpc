//! artifact 加载器。

use vos_types::{ArtifactError, OperationProjectionArtifact, ServiceProjectionArtifact};

use crate::{
    contract::HermesServiceContract,
    error::{RpcError, RpcErrorCode, RpcResult},
};

fn artifact_from_bytes<T, F>(source: &[u8], parse: F) -> RpcResult<T>
where
    F: FnOnce(&str) -> Result<T, ArtifactError>,
{
    let json = std::str::from_utf8(source)
        .map_err(|err| RpcError::new(RpcErrorCode::ArtifactCorrupt, err.to_string()))?;
    parse(json).map_err(RpcError::from_artifact)
}

/// 从 artifact 加载服务描述的 trait。
pub trait HermesServiceLoader {
    /// 解析 `HermesServiceContract` 形状的手写 artifact。
    fn load(&self, source: &[u8]) -> RpcResult<HermesServiceContract> {
        let contract: HermesServiceContract = serde_json::from_slice(source)
            .map_err(|err| RpcError::new(RpcErrorCode::InvalidRequest, err.to_string()))?;
        Ok(contract)
    }

    /// 从文件加载手写 artifact。
    fn load_file(&self, path: &std::path::Path) -> RpcResult<HermesServiceContract> {
        let bytes = std::fs::read(path)?;
        self.load(&bytes)
    }

    /// 解析 `vos-language` 产出的 service projection artifact。
    fn load_artifact(&self, source: &[u8]) -> RpcResult<ServiceProjectionArtifact> {
        artifact_from_bytes(source, ServiceProjectionArtifact::from_json)
    }

    /// 从文件加载 service projection artifact。
    fn load_artifact_file(&self, path: &std::path::Path) -> RpcResult<ServiceProjectionArtifact> {
        let bytes = std::fs::read(path)?;
        self.load_artifact(&bytes)
    }

    /// 解析 `vos-language` 产出的 operation projection artifact。
    fn load_operation_artifact(&self, source: &[u8]) -> RpcResult<OperationProjectionArtifact> {
        artifact_from_bytes(source, OperationProjectionArtifact::from_json)
    }

    /// 从文件加载 operation projection artifact。
    fn load_operation_artifact_file(
        &self,
        path: &std::path::Path,
    ) -> RpcResult<OperationProjectionArtifact> {
        let bytes = std::fs::read(path)?;
        self.load_operation_artifact(&bytes)
    }
}

/// 用 `serde_json` 解析 artifact 的默认 loader。
#[derive(Clone, Debug, Default)]
pub struct JsonServiceLoader;

impl HermesServiceLoader for JsonServiceLoader {}
