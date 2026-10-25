//! Hermes RPC 生成器。
//!
//! 输入是 `vos-language` 产出的 operation projection artifact，输出是
//! operation 级 TypeScript client、server 绑定与一次性业务模板。依赖方向
//! `vos-types` ← `hermes-generator`，不依赖 `hermes-runtime`：生成过程不执行
//! 任何 handler。
//!
//! method 级入口 [`generate_service`] 直接消费上游 `vos/hermes.contract/v0` 的
//! [`vos_types::ContractArtifact`]：按 service method 生成 identity 常量、
//! transport-neutral client、route 表、server 绑定、metadata 与一次性 handler
//! 模板。operation 级 [`generate`] 保留为底层能力，仍消费 operation projection
//! artifact 并只承认 `op_` 调用面。
//!
//! 类型映射是保守表：无法安全对应到 TS 的名字降级为 `unknown`，user 类型输出
//! nominal 引用，二者都产生诊断，不臆造更窄的类型也不会静默丢弃信息。
//!
//! 文件所有权分两类：`Generated` 每次重建，`Template` 只在缺失时创建，重复
//! 生成不覆盖用户已实现的 handler。

#![deny(missing_docs)]

mod aot;
mod bindings;
mod client;
mod naming;
mod plan;
mod server;
mod types;

pub use self::{
    aot::{AotTarget, generate_service},
    plan::{Diagnostic, FileOwnership, GeneratedFile, GenerationPlan, GeneratorError, WriteReport},
};

use vos_types::OperationProjectionArtifact;

/// 从 operation projection artifact 生成完整文件计划。
pub fn generate(artifact: &OperationProjectionArtifact) -> Result<GenerationPlan, GeneratorError> {
    let bindings = bindings::build(artifact)?;
    let files = vec![client::render(&bindings), server::render(&bindings), server::render_template(&bindings)];
    Ok(GenerationPlan::new(files, bindings.diagnostics))
}

/// 从 JSON artifact 生成完整文件计划。
pub fn generate_from_json(json: &str) -> Result<GenerationPlan, GeneratorError> {
    let artifact = OperationProjectionArtifact::from_json(json)?;
    generate(&artifact)
}
