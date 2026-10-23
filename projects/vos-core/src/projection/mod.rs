//! VOS artifact 合同镜像。
//!
//! 本模块**不编译依赖** `vos` crate：所有 `Hermes*` 结构体与上游
//! `vos_types::projection::*` 保持 bit-exact 的 serde 对齐（camelCase 字段名 +
//! `{start, end}` span + `deny_unknown_fields`），因此可以直接解析
//! `vos-language` 产出的 JSON artifact 而无需其编译依赖。
//!
//! 本模块只承载语言标准的 artifact 形状与 identity 派生。loader、registry、
//! dispatch 等 RPC 领域逻辑在 `hermes-runtime`。

mod operation;
mod service;
mod span;

pub use self::{
    operation::{
        HermesCanonicalType, HermesCanonicalTypeArgument, HermesOperationArtifact, HermesOperationKind,
        HermesOperationParameter, HermesOperationProjection, HermesSyntaxSliceContract, OPERATION_CONTRACT_FORMAT_VERSION,
        OPERATION_ID_PREFIX, OPERATION_PROJECTION_STAGE,
    },
    service::{
        HermesContractEnvelope, HermesServiceArtifact, HermesServiceProjection, HermesSourceUnit, HermesSyntaxSlice,
        SERVICE_ID_PREFIX, compute_service_id,
    },
};

/// Artifact 支持的 VOS 语言合同版本。
pub const LANGUAGE_VERSION: &str = "vos-language-v0";
/// Artifact 支持的 canonicalization 规则版本。
pub const CANONICALIZATION_VERSION: &str = "vos-canonical-v0";
/// service projection artifact 支持的 contract format version。
pub const SERVICE_CONTRACT_FORMAT_VERSION: &str = "vos-service-contract-v0";
/// 当前能消费的 service projection stage。
pub const SUPPORTED_STAGE: &str = "service-syntax-projection";
