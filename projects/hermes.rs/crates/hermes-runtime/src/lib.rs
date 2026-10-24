//! Hermes RPC ??????
//!
//! ? crate ?? Hermes ? RPC ?????artifact ????????????
//! ???dispatch ????? RPC ?????VOS projection artifact ??????
//! `vos-types`????????`vos-types` ? `hermes-runtime` ? `hermes-napi`?
//!
//! ??????? `operation_id` ???????service ???????method
//! ??????? `vos-language` ?? method projection ??????

#![deny(missing_docs)]

pub mod contract;
pub mod dispatcher;
pub mod error;
pub mod handlers;
pub mod lifecycle;
pub mod loader;
pub mod registry;
pub mod wire;

pub use self::{
    contract::{HermesMethodDescriptor, HermesServiceContract},
    dispatcher::{HermesDispatcher, HermesOperationHandler},
    error::{RpcError, RpcErrorCode, RpcResult},
    handlers::{AddHandler, EchoHandler},
    lifecycle::{CallContext, CallOptions, CancelToken},
    loader::{HermesServiceLoader, JsonServiceLoader},
    registry::{BTreeServiceRegistry, HermesServiceRegistry},
    wire::{HermesDispatchRequest, HermesDispatchResponse},
};
/// Re-export upstream projection artifact types for Hermes consumers.
pub mod projection {
    pub use vos_types::*;
}
