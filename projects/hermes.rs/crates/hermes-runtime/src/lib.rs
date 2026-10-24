//! Hermes RPC 运行时。
//!
//! 本 crate 只消费上游 `vos-types` 冻结的合同类型（`ServiceContract` /
//! `OperationContract` / artifact envelope），不解析 VOS 源文本，也不定义第二套
//! identity。职责是：
//!
//! - `contract`：加载并校验 `vos/hermes.contract/v0` envelope，保留上游诊断码；
//! - `registry`：`service 名 + method 名` → `MethodId` → `OperationId`，并做冲突检测；
//! - `dispatcher`：unary / server-stream 路由骨架，按合同 canonical type 校验
//!   请求与响应；
//! - `codec`：`ResolvedCanonicalType` 到 wire 表达的映射，未知形状显式拒绝；
//! - `lifecycle`：deadline、取消与并发上限。
//!
//! `vos-types` 是 identity、fingerprint 与 canonical type 的唯一真相源，本 crate
//! 只做重新导出，便于调用方从同一入口拿到合同作者面。

#![deny(missing_docs)]

pub mod codec;
pub mod contract;
pub mod dispatcher;
pub mod error;
pub mod handlers;
pub mod lifecycle;
pub mod loader;
pub mod registry;
pub mod wire;

pub use self::{
    codec::{WireCodec, check_value, codec_for, decode_params, decode_value, encode_value},
    contract::{HERMES_CONTRACT_FORMAT, parse_contract_artifact, read_contract_artifact, seal_hermes_artifact},
    dispatcher::{DispatchReply, HermesDispatcher, HermesOperationHandler, OperationReply},
    error::{RpcError, RpcErrorCode, RpcResult},
    handlers::{AddHandler, EchoHandler},
    lifecycle::{CallContext, CallOptions, CancelToken},
    loader::{HermesContractLoader, JsonContractLoader},
    registry::{ContractRegistry, MethodRoute},
    wire::{HermesDispatchRequest, HermesDispatchResponse},
};

/// 上游合同类型（identity、fingerprint、canonical type 与 envelope）。
pub mod contracts {
    pub use vos_types::{
        CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, MethodId, MethodIdentityKey,
        MethodStreamMode, OperationCardinality, OperationContract, OperationEffect, OperationId, OperationIdentityKey,
        OperationKind, OperationParameterContract, OperationParameterKey, ResolvedCanonicalType, ResolvedCanonicalTypeArgument,
        SemanticCapability, ServiceContract, ServiceId, ServiceIdentityKey, ServiceMethodContract,
    };
}

pub use vos_types::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, MethodId, MethodIdentityKey,
    MethodStreamMode, OperationCardinality, OperationContract, OperationEffect, OperationId, OperationIdentityKey,
    OperationKind, OperationParameterContract, OperationParameterKey, ResolvedCanonicalType, ResolvedCanonicalTypeArgument,
    SemanticCapability, ServiceContract, ServiceId, ServiceIdentityKey, ServiceMethodContract,
};
