//! artifact → TS 绑定的中间模型。
//!
//! client 与 server 两个渲染器共用同一份绑定，避免重复解析类型、重复报诊断，
//! 也保证命名冲突在生成期只检查一次。

use std::collections::BTreeMap;

use vos_core::projection::HermesOperationArtifact;

use crate::{
    naming::{camel_case, pascal_case, property_key},
    plan::{Diagnostic, GeneratorError},
    types,
};

/// 单个参数的 TS 绑定。
pub(crate) struct ParameterBinding {
    /// 属性键（合法标识符裸写，否则带引号）。
    pub(crate) key: String,
    /// TS 类型表达式。
    pub(crate) ts_type: String,
}

/// 单个 operation 的 TS 绑定。
pub(crate) struct OperationBinding {
    /// 上游原始 operation 名。
    pub(crate) raw_name: String,
    /// PascalCase 类型前缀。
    pub(crate) type_name: String,
    /// camelCase 方法名。
    pub(crate) method_name: String,
    /// 稳定 `op_` identity。
    pub(crate) operation_id: String,
    /// 参数列表。
    pub(crate) params: Vec<ParameterBinding>,
    /// 结果 TS 类型。
    pub(crate) result: String,
}

impl OperationBinding {
    /// `<Type>Params` 类型别名。
    pub(crate) fn params_type(&self) -> String {
        format!("{}Params", self.type_name)
    }

    /// `<Type>Result` 类型别名。
    pub(crate) fn result_type(&self) -> String {
        format!("{}Result", self.type_name)
    }
}

/// 完整绑定集。
pub(crate) struct Bindings {
    pub(crate) operations: Vec<OperationBinding>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

/// 从 artifact 构建绑定，冲突直接拒绝。
pub(crate) fn build(artifact: &HermesOperationArtifact) -> Result<Bindings, GeneratorError> {
    let mut operations = Vec::with_capacity(artifact.operations.len());
    let mut diagnostics = Vec::new();
    let mut by_type_name: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for operation in &artifact.operations {
        let type_name = pascal_case(&operation.name);
        by_type_name.entry(type_name.clone()).or_default().push(operation.name.clone());

        let mut params = Vec::with_capacity(operation.parameters.len());
        for parameter in &operation.parameters {
            let resolved = types::resolve(&parameter.type_expr);
            for name in resolved.unresolved {
                diagnostics.push(Diagnostic::new(format!(
                    "operation `{}` parameter `{}`: type `{name}` cannot be resolved to a TypeScript type, emitted as `unknown`",
                    operation.name, parameter.name
                )));
            }
            params.push(ParameterBinding { key: property_key(&parameter.name), ts_type: resolved.ts });
        }

        let result = match &operation.return_type {
            Some(return_type) => {
                let resolved = types::resolve(return_type);
                for name in resolved.unresolved {
                    diagnostics.push(Diagnostic::new(format!(
                        "operation `{}` return type `{name}` cannot be resolved to a TypeScript type, emitted as `unknown`",
                        operation.name
                    )));
                }
                resolved.ts
            }
            None => types::NO_RETURN_TYPE.to_string(),
        };

        operations.push(OperationBinding {
            raw_name: operation.name.clone(),
            method_name: camel_case(&operation.name),
            type_name,
            operation_id: operation.operation_id.clone(),
            params,
            result,
        });
    }

    for (identifier, names) in by_type_name {
        if names.len() > 1 {
            return Err(GeneratorError::NamingCollision { identifier, operations: names });
        }
    }

    Ok(Bindings { operations, diagnostics })
}
