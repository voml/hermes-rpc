//! method 级 AOT 生成入口。
//!
//! 输入是上游 `vos/hermes.contract/v0` 的 [`ContractArtifact`]，输出是普通
//! TypeScript 源码：service / method identity 常量、transport-neutral client、
//! route 表、server handler 绑定、metadata 与一次性业务模板。生成物不写入任何
//! Hermes runtime 依赖，宿主协议与框架由目标项目自行选择。
//!
//! 全部 method 级文件落在 [`OUTPUT_DIR`] 子目录，与 operation 级的
//! `hermes/operations.ts`、`hermes/server.ts`、`hermes/handlers.ts` 不共享任何
//! 路径，两类计划可以写进同一个项目而不会互相覆盖。
//!
//! operation 级 [`crate::generate`] 保留为底层能力：它消费 operation projection
//! artifact，本模块只承认 service method 调用面。

use std::collections::{BTreeMap, BTreeSet};

use vos_types::{
    ContractArtifact, ContractFormat, MethodStreamMode, ServiceContract, ServiceMethodContract, service_contract_fingerprint,
};

use crate::{
    naming::{camel_case, pascal_case},
    plan::{Diagnostic, GenerationPlan, GeneratorError},
};

mod client;
mod server;
mod service;
mod types;

/// AOT 宿主 profile。
///
/// 桌面与 Web 的差异只在 server-stream 的 TS 形状：桌面宿主用 `AsyncIterable`，
/// Web 宿主用 `ReadableStream`。两种 profile 的生成物互不 import 对方宿主。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AotTarget {
    /// 桌面 N-API server 宿主。
    Desktop,
    /// Web / WebAssembly 宿主。
    Web,
}

impl AotTarget {
    /// 稳定 profile 名，用于 metadata 与生成物注释。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Web => "web",
        }
    }

    /// server-stream 方法在 client 与 server 两侧使用的 TS 流类型。
    fn stream_type(self) -> &'static str {
        match self {
            Self::Desktop => "AsyncIterable",
            Self::Web => "ReadableStream",
        }
    }
}

/// method 级生成物的独立子目录。
///
/// operation 级占用 `hermes/operations.ts`、`hermes/server.ts` 与
/// `hermes/handlers.ts`；method 级不得复用其中任何同名文件，否则两类计划写进
/// 同一项目时会互相覆盖，`Template` 的一次性语义也随之失效。
pub(crate) const OUTPUT_DIR: &str = "hermes/service";

/// method 级生成物的完整相对路径。
pub(crate) fn output_path(file_name: &str) -> String {
    format!("{OUTPUT_DIR}/{file_name}")
}

/// 从 `vos/hermes.contract/v0` artifact 生成 method 级文件计划。
///
/// 非 Hermes 格式与不满足上游合同校验的 artifact 直接拒绝。类型无法完整映射时
/// 返回结构化 [`GeneratorError`]，不会降级成 `unknown` 后宣称生成成功。
pub fn generate_service(artifact: &ContractArtifact, target: AotTarget) -> Result<GenerationPlan, GeneratorError> {
    if artifact.version != ContractFormat::Hermes {
        return Err(GeneratorError::UnsupportedContractFormat { found: artifact.version.as_str() });
    }
    artifact.validate()?;

    let bindings = build(artifact)?;
    let files = vec![
        service::render_service(&bindings),
        client::render(&bindings, target),
        service::render_routes(&bindings),
        server::render(&bindings, target),
        service::render_metadata(&bindings, artifact, target),
        server::render_template(&bindings),
    ];

    Ok(GenerationPlan::new(files, bindings.diagnostics))
}

/// 单个 method 的 TS 绑定。
pub(crate) struct MethodBinding {
    /// 上游原始方法名。
    pub(crate) raw_name: String,
    /// camelCase 方法标识符（client 方法、handler 键）。
    pub(crate) ts_name: String,
    /// durable `mtd_` identity。
    pub(crate) method_id: String,
    /// 绑定的 durable `op_` identity。
    pub(crate) operation_id: String,
    /// 调用模式。
    pub(crate) stream: MethodStreamMode,
    /// 请求 TS 类型。
    pub(crate) request: String,
    /// 响应 TS 类型；server-stream 时是元素类型。
    pub(crate) response: String,
    /// 按上游 service fingerprint 规则导出的 method fingerprint。
    pub(crate) fingerprint: String,
    /// route 表键：`<serviceId>/<methodName>`。
    pub(crate) route_key: String,
}

/// 单个 service 的 TS 绑定。
pub(crate) struct ServiceBinding {
    /// 上游原始 service 名。
    pub(crate) raw_name: String,
    /// camelCase service 键。
    pub(crate) key: String,
    /// client 类名。
    pub(crate) class_name: String,
    /// durable `svc_` identity。
    pub(crate) service_id: String,
    /// canonical path。
    pub(crate) canonical_path: Vec<String>,
    /// service fingerprint。
    pub(crate) fingerprint: String,
    /// 按声明顺序排列的方法。
    pub(crate) methods: Vec<MethodBinding>,
}

/// 全部绑定与诊断。
pub(crate) struct Bindings {
    pub(crate) services: Vec<ServiceBinding>,
    pub(crate) user_types: Vec<types::UserTypeBinding>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

fn build(artifact: &ContractArtifact) -> Result<Bindings, GeneratorError> {
    let contract = &artifact.payload;
    let operation_ids = contract.operations.iter().map(|operation| operation.operation_id.as_str()).collect::<BTreeSet<_>>();

    let mut users = types::UserTypes::default();
    let mut diagnostics = Vec::new();
    let mut services = Vec::with_capacity(contract.services.len());
    let mut service_keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut routes = BTreeSet::new();

    for service in &contract.services {
        let key = camel_case(&service.name);
        service_keys.entry(key.clone()).or_default().push(service.name.clone());

        let mut methods = Vec::with_capacity(service.methods.len());
        for method in &service.methods {
            if !operation_ids.contains(method.operation_id.as_str()) {
                return Err(GeneratorError::MissingOperationBinding {
                    service: service.name.clone(),
                    method: method.name.clone(),
                    operation_id: method.operation_id.to_string(),
                });
            }

            let route = format!("{key}.{}", camel_case(&method.name));
            if !routes.insert(route.clone()) {
                return Err(GeneratorError::DuplicateRoute { route: format!("{}.{}", service.name, camel_case(&method.name)) });
            }

            let (request, response) = map_method_types(service, method, &mut users, &mut diagnostics)?;
            methods.push(MethodBinding {
                raw_name: method.name.clone(),
                ts_name: camel_case(&method.name),
                method_id: method.method_id.to_string(),
                operation_id: method.operation_id.to_string(),
                stream: method.stream,
                request,
                response,
                fingerprint: method_fingerprint(service, method),
                route_key: format!("{}/{}", service.service_id, method.name),
            });
        }

        services.push(ServiceBinding {
            raw_name: service.name.clone(),
            key,
            class_name: format!("{}Client", pascal_case(&service.name)),
            service_id: service.service_id.to_string(),
            canonical_path: service.canonical_path.clone(),
            fingerprint: service_fingerprint(service),
            methods,
        });
    }

    if let Some((identifier, sources)) = service_keys.iter().find(|(_, names)| names.len() > 1) {
        return Err(GeneratorError::IdentifierCollision { identifier: identifier.clone(), sources: sources.clone() });
    }
    if let Some((identifier, sources)) = users.collision() {
        return Err(GeneratorError::NamingCollision { identifier, operations: sources });
    }

    dedupe(&mut diagnostics);

    Ok(Bindings { services, user_types: users.into_bindings(), diagnostics })
}

fn map_method_types(
    service: &ServiceContract,
    method: &ServiceMethodContract,
    users: &mut types::UserTypes,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(String, String), GeneratorError> {
    let request = types::map(&method.request_type, users)
        .map_err(|error| annotate(error, &format!("method `{}.{}` request", service.name, method.name)))?;
    for issue in request.issues {
        diagnostics.push(Diagnostic::new(format!("method `{}.{}` request: {issue}", service.name, method.name)));
    }

    let response = types::map(&method.response_type, users)
        .map_err(|error| annotate(error, &format!("method `{}.{}` response", service.name, method.name)))?;
    for issue in response.issues {
        diagnostics.push(Diagnostic::new(format!("method `{}.{}` response: {issue}", service.name, method.name)));
    }

    Ok((request.ts, response.ts))
}

/// 给结构化类型映射错误补上 service / method 上下文，保留原始错误码。
fn annotate(error: GeneratorError, context: &str) -> GeneratorError {
    match error {
        GeneratorError::Artifact(mut error) => {
            error.message = format!("{context}: {}", error.message);
            GeneratorError::Artifact(error)
        }
        other => other,
    }
}

/// service fingerprint 直接复用上游规则，不另造 identity。
pub(crate) fn service_fingerprint(service: &ServiceContract) -> String {
    service_contract_fingerprint(std::slice::from_ref(service))
}

/// method fingerprint 复用上游规则：只保留该 method 的 service 视图再取指纹。
pub(crate) fn method_fingerprint(service: &ServiceContract, method: &ServiceMethodContract) -> String {
    let mut scoped = service.clone();
    scoped.methods = vec![method.clone()];
    service_contract_fingerprint(std::slice::from_ref(&scoped))
}

/// 生成物里 `stream` 字段的稳定拼写。
pub(crate) fn stream_name(stream: MethodStreamMode) -> &'static str {
    match stream {
        MethodStreamMode::Unary => "unary",
        MethodStreamMode::ServerStream => "server-stream",
    }
}

/// 生成物里 method 的完整 TS 返回类型。
pub(crate) fn stream_result(target: AotTarget, stream: MethodStreamMode, response: &str) -> String {
    match stream {
        MethodStreamMode::Unary => format!("Promise<{response}>"),
        MethodStreamMode::ServerStream => format!("{}<{response}>", target.stream_type()),
    }
}

/// 生成物里 server handler 的返回类型：unary 允许同步或异步。
pub(crate) fn handler_result(target: AotTarget, stream: MethodStreamMode, response: &str) -> String {
    match stream {
        MethodStreamMode::Unary => format!("Promise<{response}> | {response}"),
        MethodStreamMode::ServerStream => format!("{}<{response}>", target.stream_type()),
    }
}

/// 生成物里 method 的 dispatch 调用名。
pub(crate) fn dispatch_name(stream: MethodStreamMode) -> &'static str {
    match stream {
        MethodStreamMode::Unary => "unary",
        MethodStreamMode::ServerStream => "serverStream",
    }
}

fn dedupe(diagnostics: &mut Vec<Diagnostic>) {
    let mut seen = BTreeSet::new();
    diagnostics.retain(|diagnostic| seen.insert(diagnostic.message().to_string()));
}
