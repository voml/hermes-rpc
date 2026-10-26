//! 合同 registry：`service 名 + method 名` → `MethodId` → `OperationId`。
//!
//! service、method 与 operation 的 identity 全部是上游 `vos-types` 的 durable id。
//! registry 只做解析、冲突检测与查找，不推导 identity，也不会在查找失败时按名字
//! 猜一个相近条目：未知 service / method 返回结构化错误。
//!
//! 注册是原子的：先完成全部冲突检查，再一次性写入，避免半套合同进入 registry。

use std::collections::{BTreeMap, BTreeSet};

use vos_types::{
    ContractArtifact, ContractFormat, MethodId, MethodStreamMode, OperationContract, OperationId, ServiceContract, ServiceId,
    ServiceMethodContract,
};

use crate::{
    contract::HERMES_CONTRACT_FORMAT,
    error::{RpcError, RpcErrorCode, RpcResult},
};

/// 一次 `service.method` 解析的结果。
#[derive(Clone, Copy, Debug)]
pub struct MethodRoute<'a> {
    /// 拥有该方法的 service 合同。
    pub service: &'a ServiceContract,
    /// 方法合同（携带 `MethodId` 与绑定的 `OperationId`）。
    pub method: &'a ServiceMethodContract,
    /// 方法绑定的 operation 合同。
    pub operation: &'a OperationContract,
}

impl MethodRoute<'_> {
    /// 方法的 durable identity。
    pub fn method_id(&self) -> &MethodId {
        &self.method.method_id
    }

    /// 方法绑定的 operation durable identity。
    pub fn operation_id(&self) -> &OperationId {
        &self.method.operation_id
    }

    /// 方法的调用模式。
    pub fn stream(&self) -> MethodStreamMode {
        self.method.stream
    }
}

/// 以 durable identity 为 key 的合同 registry。
#[derive(Clone, Debug, Default)]
pub struct ContractRegistry {
    services: BTreeMap<ServiceId, ServiceContract>,
    operations: BTreeMap<OperationId, OperationContract>,
    service_names: BTreeMap<String, ServiceId>,
    /// `MethodId` → `OperationId` 反向索引，供 handler 注册按 method 名查找 operation 使用。
    method_index: BTreeMap<MethodId, OperationId>,
}

impl ContractRegistry {
    /// 构造空 registry。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一份 Hermes 合同 artifact。
    ///
    /// 拒绝非 Hermes 格式、重复 service 名、重复 service / method / operation id、
    /// 绑定了 payload 中不存在 operation 的 method，以及尚未通过上游校验的
    /// artifact（陈旧 fingerprint、identity 与 canonical path 不一致、payload
    /// 内部重复）。所有检查都在写入前完成，损坏 artifact 不会半套进入 registry。
    pub fn register_artifact(&mut self, artifact: &ContractArtifact) -> RpcResult<()> {
        if artifact.version != ContractFormat::Hermes {
            return Err(RpcError::new(
                RpcErrorCode::ArtifactCorrupt,
                format!("registry accepts only `{HERMES_CONTRACT_FORMAT}`, found `{}`", artifact.version.as_str()),
            )
            .with_diagnostic_code("ART010"));
        }

        let mut incoming_services: BTreeMap<ServiceId, ServiceContract> = BTreeMap::new();
        let mut incoming_operations: BTreeMap<OperationId, OperationContract> = BTreeMap::new();
        let mut incoming_names: BTreeMap<String, ServiceId> = BTreeMap::new();
        let mut incoming_method_index: BTreeMap<MethodId, OperationId> = BTreeMap::new();

        for operation in &artifact.payload.operations {
            if self.operations.contains_key(&operation.operation_id)
                || incoming_operations.contains_key(&operation.operation_id)
            {
                return Err(duplicate("operation", operation.operation_id.as_str()));
            }
            incoming_operations.insert(operation.operation_id.clone(), operation.clone());
        }

        for service in &artifact.payload.services {
            if service.name.trim().is_empty() {
                return Err(RpcError::new(RpcErrorCode::ArtifactCorrupt, "service name must not be empty"));
            }
            if self.services.contains_key(&service.service_id) || incoming_services.contains_key(&service.service_id) {
                return Err(duplicate("service id", service.service_id.as_str()));
            }
            if self.service_names.contains_key(&service.name) || incoming_names.contains_key(&service.name) {
                return Err(duplicate("service name", &service.name));
            }
            incoming_names.insert(service.name.clone(), service.service_id.clone());

            let mut method_names: BTreeSet<&str> = BTreeSet::new();
            for method in &service.methods {
                if method.name.trim().is_empty() {
                    return Err(RpcError::new(
                        RpcErrorCode::ArtifactCorrupt,
                        format!("service `{}` has an empty method name", service.name),
                    ));
                }
                if !method_names.insert(method.name.as_str()) {
                    return Err(duplicate("method name", &format!("{}.{}", service.name, method.name)));
                }
                // MethodId 唯一性：同一个 registry 中同一个 MethodId 不能映射到两个 Operation。
                // 先检查 self.method_index（已挂载 artifact），再检查 incoming_method_index（当前 artifact 内部）。
                if self.method_index.contains_key(&method.method_id)
                    || incoming_method_index.contains_key(&method.method_id)
                {
                    return Err(duplicate("method id", method.method_id.as_str()));
                }
                if !incoming_operations.contains_key(&method.operation_id)
                    && !self.operations.contains_key(&method.operation_id)
                {
                    return Err(RpcError::new(
                        RpcErrorCode::MissingOperationBinding,
                        format!(
                            "method `{}.{}` binds operation `{}` which is not present in the contract",
                            service.name, method.name, method.operation_id
                        ),
                    ));
                }
                incoming_method_index.insert(method.method_id.clone(), method.operation_id.clone());
            }
            incoming_services.insert(service.service_id.clone(), service.clone());
        }

        // 上游校验放在最后、写入之前：陈旧的 payload fingerprint 与 identity
        // 不一致的 artifact 必须在挂载期失败，而不是等到 dispatch 才暴露。
        artifact.validate()?;

        self.operations.extend(incoming_operations);
        self.service_names.extend(incoming_names);
        self.services.extend(incoming_services);
        self.method_index.extend(incoming_method_index);
        Ok(())
    }

    /// 按 service 名 + method 名解析到 method 与 operation 合同。
    pub fn resolve(&self, service_name: &str, method_name: &str) -> RpcResult<MethodRoute<'_>> {
        let service_id = self.service_names.get(service_name).ok_or_else(|| {
            RpcError::new(RpcErrorCode::UnknownService, format!("service `{service_name}` is not registered"))
        })?;
        let service = self.services.get(service_id).ok_or_else(|| {
            RpcError::new(RpcErrorCode::UnknownService, format!("service `{service_name}` is not registered"))
        })?;
        let method = service.methods.iter().find(|method| method.name == method_name).ok_or_else(|| {
            RpcError::new(RpcErrorCode::UnknownMethod, format!("service `{service_name}` has no method `{method_name}`"))
        })?;
        let operation = self.operations.get(&method.operation_id).ok_or_else(|| {
            RpcError::new(
                RpcErrorCode::MissingOperationBinding,
                format!(
                    "method `{service_name}.{method_name}` binds operation `{}` which is not registered",
                    method.operation_id
                ),
            )
        })?;
        Ok(MethodRoute { service, method, operation })
    }

    /// 按 service 名 + method 名直接查对应的 `OperationId`。
    ///
    /// 内部复用 [`Self::resolve`] 以保持错误消息一致。
    pub fn operation_for_method(&self, service_name: &str, method_name: &str) -> RpcResult<OperationId> {
        let route = self.resolve(service_name, method_name)?;
        Ok(route.method.operation_id.clone())
    }

    /// 按 durable `MethodId` 查绑定的 `OperationId`。
    pub fn operation_id_for_method(&self, method_id: &MethodId) -> Option<&OperationId> {
        self.method_index.get(method_id)
    }

    /// 按 service 名查找 service 合同。
    pub fn service(&self, service_name: &str) -> Option<&ServiceContract> {
        self.service_names.get(service_name).and_then(|id| self.services.get(id))
    }

    /// 按 durable id 查找 operation 合同。
    pub fn operation(&self, operation_id: &OperationId) -> Option<&OperationContract> {
        self.operations.get(operation_id)
    }

    /// 已注册 service 的数量。
    pub fn service_count(&self) -> usize {
        self.services.len()
    }

    /// 已注册 service 名（按名字排序）。
    pub fn service_names(&self) -> Vec<&str> {
        self.service_names.keys().map(String::as_str).collect()
    }

    /// 已注册 operation 的数量。
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }
}

fn duplicate(kind: &str, value: &str) -> RpcError {
    RpcError::new(RpcErrorCode::DuplicateRegistration, format!("{kind} `{value}` already registered"))
}
