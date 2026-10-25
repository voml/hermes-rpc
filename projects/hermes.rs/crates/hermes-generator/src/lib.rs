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
//! 模板。method 级文件全部落在 `hermes/service/` 子目录，operation 级
//! [`generate`] 保留为底层能力，仍消费 operation projection artifact、只承认
//! `op_` 调用面并占用 `hermes/operations.ts`、`hermes/server.ts` 与
//! `hermes/handlers.ts`。两类计划写进同一项目时路径无交集，互不覆盖。
//!
//! method 级类型映射不降级：标量与包装逐项转写，user 类型输出 nominal 引用并
//! 产生诊断，未知 generic、literal 参数与未冻结的 `json` / `value` / `any` 直接
//! 返回结构化 `GeneratorError`。operation 级临时映射表仍对未解析名字保留
//! `unknown` 并报诊断。
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
