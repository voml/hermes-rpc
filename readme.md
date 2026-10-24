# Hermes RPC

Hermes 是 VMZ 体系中的 RPC 协议宿主与服务消费端。

## 当前状态

**旧栈已冻结。** 2026-10-23 之前的 `vos-ast` / `vos-for-rust` / `vos-openapi` / `vos-cli` / `vos-faker` 原型已从 workspace 移除——它们依赖的 `diagnostic-quick 0.4.0` 与当前 `diagnostic` crate 不兼容，且 Hermes 已决定**不再自建 parser/AST**。

**新主链**：Hermes 消费 [`vos-language`](https://github.com/voml/vos-language) 产出的 frozen contract artifact（`vos/hermes.contract/v0`），按其中的 `ServiceContract` / `OperationContract`、`MethodId` 与 `OperationId` 注册并路由 RPC 调用。

## 工作区结构

```
hermes-rpc/
├── Cargo.toml                      # workspace 入口
├── projects/
│   ├── hermes.rs/crates/           # Rust：runtime / generator / napi
│   └── hermes.ts/
│       ├── packages/               # `@yydb/hermes`、skills
│       └── platforms/              # 各平台 N-API addon
```

## 上游 `vos-types`（`vos-language`）

Hermes **不** vendor 本地镜像 crate；contract artifact 类型、identity、fingerprint 与 canonical type 直接依赖上游 `vos-types`（git `dev`，本地开发可用 `.cargo/config.toml` path patch）。

- **artifact 合同**：`ContractArtifact` / `ServiceContract` / `OperationContract`，直接复用上游 serde 形状与校验，未知字段、未知版本、指纹不匹配与 producer 缺失由上游诊断码（`ART010` / `ART020` / `ART021`）拒绝。
- **错误层**：`ArtifactError` 在 RPC 边界映射为 `RpcError`，上游诊断码原样保留在 `diagnostic_code`。

## `hermes-runtime` 提供

- `parse_contract_artifact` / `JsonContractLoader`——只接受 `vos/hermes.contract/v0`，不解析 VOS 源文本
- `ContractRegistry`——`service 名 + method 名` → `MethodId` → `OperationId`，重复 service / method / operation 与悬空 method 绑定在注册期拒绝，查找失败返回结构化错误
- `HermesDispatcher` / `HermesOperationHandler`——按解析出的 `operation_id` 路由到用户 handler，`OperationReply` 区分 unary 与 server-stream
- `WireCodec` / `codec_for`——`ResolvedCanonicalType` 的 wire 表达，`utf8` 与 `utf16` 不合并，未声明 generic 直接拒绝
- `HermesDispatchRequest` / `HermesDispatchResponse`——请求携带 `service` + `method` + `params`，响应回填解析出的 durable id
- `RpcError` / `RpcErrorCode`——稳定错误码族，wire 错误体为 `{"code": "...", "message": "..."}`，artifact 错误额外带 `diagnosticCode`
- `CallOptions` / `CancelToken` / `CallContext`——dispatch 生命周期：deadline、取消与在途并发上限

## `hermes-generator` 提供

- `generate` / `generate_from_json`——从 operation artifact 产出 `GenerationPlan`
- `hermes/operations.ts`——operation id 常量、参数 / 返回类型与 `createHermesClient(transport)`
- `hermes/server.ts`——按 `operation_id` 绑定 handler 的 `bindOperationHandlers()`
- `hermes/handlers.ts`——一次性业务模板，已存在则保留（`FileOwnership::Template`）
- `Diagnostic`——无法安全映射的类型降级为 `unknown` 并记录诊断，命名冲突直接拒绝

生成物不 import 任何 Hermes npm 包，只声明 `HermesTransport` 接口，由宿主运行时绑定传入。

## 接入 `vos-language`

Hermes 不编译依赖 `vos` crate；它只消费 `vos-language` 产出的 contract artifact JSON。字段命名、identity、fingerprint 与 canonical type 以上游 `vos-types` 为准，集成测试直接加载上游 `operations_services.contract.json` fixture 并重新封装成 Hermes 格式，证明 payload 逐字段等价。

## 验证

```bash
cargo test --workspace
```

53 个测试全部通过：28 个 runtime 合同 / registry / dispatch 测试 + 10 个 generator 测试 + 15 个 wasm binding 测试。

## 设计约束

- **Hermes 不解析 VOS 源码**：VOS 文本 → ServiceContract artifact 在 `vos-language` 侧完成；Hermes 只消费 artifact。
- **1.0.0 前不保留旧 API 兼容层**：旧原型一旦无法编译就直接移除，不做 wrapper、alias、fallback 或双读双写。
- **错误自定**：Hermes 运行时错误模型独立于 `vos-language` 上游，避免跨仓耦合。
