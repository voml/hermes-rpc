//! Hermes RPC 生成器。
//!
//! 输入是 `vos-language` 产出的 operation projection artifact，输出是
//! operation 级 TypeScript client、server 绑定与一次性业务模板。依赖方向
//! `vos-types` ← `hermes-generator`，不依赖 `hermes-runtime`：生成过程不执行
//! 任何 handler。
//!
//! 当前阶段上游尚未交付 method 级 projection，因此生成物只承认 operation 级
//! 调用（`op_` identity + 声明参数 + 声明返回类型）。类型映射是临时表：无法
//! 安全对应到 TS 原语的名字降级为 `unknown` 并输出诊断，不臆造更窄的类型。
//!
//! 文件所有权分两类：`Generated` 每次重建，`Template` 只在缺失时创建，重复
//! 生成不覆盖用户已实现的 handler。

#![deny(missing_docs)]

mod bindings;
mod client;
mod naming;
mod plan;
mod server;
mod types;

pub use self::plan::{Diagnostic, FileOwnership, GeneratedFile, GenerationPlan, GeneratorError, WriteReport};

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
