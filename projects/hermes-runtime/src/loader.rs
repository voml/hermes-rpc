//! artifact 加载器。
//!
//! 加载器只做 parse + 版本校验，不持有状态。版本与 span 校验由
//! `vos_core::projection` 的 artifact 类型负责。

use vos_core::{
    error::QResult,
    projection::{HermesOperationArtifact, HermesServiceArtifact},
};

use crate::contract::HermesServiceContract;

/// 从 artifact 加载服务描述的 trait。
///
/// 默认实现同时覆盖：
/// - `load` / `load_file` —— 解析 `HermesServiceContract` 形状的手写 artifact
/// - `load_artifact` / `load_artifact_file` —— 解析 `vos-language` 产出的
///   `ServiceProjectionArtifact`（真实消费主链）
/// - `load_operation_artifact` / `load_operation_artifact_file` —— 解析
///   `vos-language` 产出的 `OperationProjectionArtifact`
pub trait HermesServiceLoader {
    /// 解析 `HermesServiceContract` 形状的手写 artifact。
    fn load(&self, source: &[u8]) -> QResult<HermesServiceContract> {
        let contract: HermesServiceContract = serde_json::from_slice(source)?;
        Ok(contract)
    }

    /// 从文件加载手写 artifact。
    fn load_file(&self, path: &std::path::Path) -> QResult<HermesServiceContract> {
        let bytes = std::fs::read(path)?;
        self.load(&bytes)
    }

    /// 解析 `vos-language` 产出的 service projection artifact。
    fn load_artifact(&self, source: &[u8]) -> QResult<HermesServiceArtifact> {
        HermesServiceArtifact::from_slice(source)
    }

    /// 从文件加载 service projection artifact。
    fn load_artifact_file(&self, path: &std::path::Path) -> QResult<HermesServiceArtifact> {
        let bytes = std::fs::read(path)?;
        self.load_artifact(&bytes)
    }

    /// 解析 `vos-language` 产出的 operation projection artifact。
    fn load_operation_artifact(&self, source: &[u8]) -> QResult<HermesOperationArtifact> {
        HermesOperationArtifact::from_slice(source)
    }

    /// 从文件加载 operation projection artifact。
    fn load_operation_artifact_file(&self, path: &std::path::Path) -> QResult<HermesOperationArtifact> {
        let bytes = std::fs::read(path)?;
        self.load_operation_artifact(&bytes)
    }
}

/// 用 `serde_json` 解析 artifact 的默认 loader。
#[derive(Clone, Debug, Default)]
pub struct JsonServiceLoader;

impl HermesServiceLoader for JsonServiceLoader {}
