//! 服务与 operation 注册表。
//!
//! services 与 operations 分开存储：services 的 key 是全限定服务名，
//! operations 的 key 是上游给出的稳定 `operation_id`（`op_` + 16 hex）。
//! 重复 identity 在注册期直接拒绝，不留到 dispatch 期。

use std::collections::BTreeMap;

use vos_core::projection::{
    HermesOperationArtifact, HermesOperationProjection, HermesServiceArtifact, HermesServiceProjection,
};

use crate::{
    contract::{HermesMethodDescriptor, HermesServiceContract},
    error::{RpcError, RpcErrorCode, RpcResult},
};

/// 注册与查找已加载服务描述的 trait。
pub trait HermesServiceRegistry {
    /// 注册服务描述。
    fn register(&mut self, contract: HermesServiceContract) -> RpcResult<()> {
        let name = contract.service_name.clone();
        self.insert_contract(name, contract)
    }

    /// 按全限定名查找已注册的服务。
    fn find(&self, service_name: &str) -> Option<&HermesServiceContract>;

    /// 返回所有已注册服务的迭代。
    fn all(&self) -> Vec<&HermesServiceContract>;

    /// 按服务名 + 方法名查找已注册的方法。
    fn method(&self, service_name: &str, method_name: &str) -> Option<&HermesMethodDescriptor> {
        self.find(service_name).and_then(|c| c.methods.iter().find(|m| m.name == method_name))
    }

    /// 从 `HermesServiceArtifact` 批量注册服务描述。
    fn register_artifact(&mut self, artifact: &HermesServiceArtifact) -> RpcResult<()> {
        for projection in &artifact.services {
            self.register_projection(projection)?;
        }
        Ok(())
    }

    /// 把单个 `HermesServiceProjection` 注册为服务描述。
    ///
    /// 当前 projection 阶段（`service-syntax-projection`）尚未携带 methods，
    /// 因此转换得到的 `HermesServiceContract.methods` 为空。将来更低阶段的
    /// projection 可以覆盖这个默认行为。
    fn register_projection(&mut self, projection: &HermesServiceProjection) -> RpcResult<()> {
        let service_name =
            if !projection.canonical_path.is_empty() { projection.canonical_path.join(".") } else { projection.name.clone() };
        let contract = HermesServiceContract { service_name, methods: vec![], revision: None };
        self.register(contract)
    }

    /// 从 `HermesOperationArtifact` 批量注册 operation。
    ///
    /// Operations 是顶层声明，不属于某个 service——Hermes 独立维护
    /// operations registry（key = operation_id），与 services 分开存储。
    fn register_operation_artifact(&mut self, artifact: &HermesOperationArtifact) -> RpcResult<()> {
        for operation in &artifact.operations {
            self.register_operation(operation)?;
        }
        Ok(())
    }

    /// 注册单个 operation projection 到独立的 operations registry。
    fn register_operation(&mut self, projection: &HermesOperationProjection) -> RpcResult<()> {
        self.insert_operation(projection.operation_id.clone(), projection.clone())
    }

    /// 按 operation_id 查找已注册的 operation。
    fn find_operation(&self, operation_id: &str) -> Option<&HermesOperationProjection>;

    /// 按 name 查找已注册的 operation（取第一个匹配）。
    fn find_operation_by_name(&self, name: &str) -> Option<&HermesOperationProjection>;

    /// 返回所有已注册 operation 的迭代。
    fn all_operations(&self) -> Vec<&HermesOperationProjection>;

    /// 内部钩子：由具体实现提供实际的服务存储逻辑。
    fn insert_contract(&mut self, name: String, contract: HermesServiceContract) -> RpcResult<()>;

    /// 内部钩子：由具体实现提供实际的 operation 存储逻辑。
    fn insert_operation(&mut self, operation_id: String, projection: HermesOperationProjection) -> RpcResult<()>;
}

/// 基于 `BTreeMap` 的简单 registry，适合单机 Hermes 进程。
///
/// 独立维护 services 和 operations 两个 map：
/// - services: key = 全限定服务名（canonical path join）
/// - operations: key = operation_id（`op_` + 16 hex）
#[derive(Clone, Debug, Default)]
pub struct BTreeServiceRegistry {
    services: BTreeMap<String, HermesServiceContract>,
    operations: BTreeMap<String, HermesOperationProjection>,
}

impl BTreeServiceRegistry {
    /// 按全限定名（canonical path join）或原始名查找 projection artifact 中的服务。
    pub fn find_by_name_or_canonical(&self, projection: &HermesServiceProjection) -> Option<&HermesServiceContract> {
        let canonical =
            if !projection.canonical_path.is_empty() { projection.canonical_path.join(".") } else { projection.name.clone() };
        self.find(&canonical).or_else(|| self.find(&projection.name))
    }
}

impl HermesServiceRegistry for BTreeServiceRegistry {
    fn insert_contract(&mut self, name: String, contract: HermesServiceContract) -> RpcResult<()> {
        if self.services.contains_key(&name) {
            return Err(RpcError::new(RpcErrorCode::DuplicateRegistration, format!("service `{name}` already registered")));
        }
        self.services.insert(name, contract);
        Ok(())
    }

    fn insert_operation(&mut self, operation_id: String, projection: HermesOperationProjection) -> RpcResult<()> {
        if self.operations.contains_key(&operation_id) {
            return Err(RpcError::new(
                RpcErrorCode::DuplicateRegistration,
                format!("operation `{}` ({operation_id}) already registered", projection.name),
            ));
        }
        self.operations.insert(operation_id, projection);
        Ok(())
    }

    fn find(&self, service_name: &str) -> Option<&HermesServiceContract> {
        self.services.get(service_name)
    }

    fn all(&self) -> Vec<&HermesServiceContract> {
        self.services.values().collect()
    }

    fn find_operation(&self, operation_id: &str) -> Option<&HermesOperationProjection> {
        self.operations.get(operation_id)
    }

    fn find_operation_by_name(&self, name: &str) -> Option<&HermesOperationProjection> {
        self.operations.values().find(|op| op.name == name)
    }

    fn all_operations(&self) -> Vec<&HermesOperationProjection> {
        self.operations.values().collect()
    }
}
