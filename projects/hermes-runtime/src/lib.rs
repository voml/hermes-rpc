//! Hermes RPC 领域运行时。
//!
//! 本 crate 承接 Hermes 的 RPC 领域职责：artifact 的加载与注册、服务与方法
//! 路由、dispatch 生命周期与 RPC 错误分类。VOS 语言层与 VOS artifact 合同镜像
//! 留在 `vos-core`，依赖方向单向：`vos-core` ← `hermes-runtime` ← `hermes-napi`。
//!
//! 当前实现仍是按 `operation_id` 直路由的原型：service 只是注册容器，method
//! 级路由要等上游 `vos-language` 交付 method projection 后才能成立。

#![deny(missing_docs)]

pub mod contract;
pub mod dispatcher;
pub mod handlers;
pub mod loader;
pub mod registry;
pub mod wire;

pub use self::{
    contract::{HermesMethodDescriptor, HermesServiceContract},
    dispatcher::{HermesDispatcher, HermesOperationHandler},
    handlers::{AddHandler, EchoHandler},
    loader::{HermesServiceLoader, JsonServiceLoader},
    registry::{BTreeServiceRegistry, HermesServiceRegistry},
    wire::{HermesDispatchRequest, HermesDispatchResponse},
};
pub use vos_core::{error, projection};
