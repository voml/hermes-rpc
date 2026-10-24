# Hermes RPC

Hermes 是 VMZ 体系中的 RPC 协议宿主与服务消费端。

## 当前状态

**旧栈已冻结。** 2026-10-23 之前的 `vos-ast` / `vos-for-rust` / `vos-openapi` / `vos-cli` / `vos-faker` 原型已从 workspace 移除——它们依赖的 `diagnostic-quick 0.4.0` 与当前 `diagnostic` crate 不兼容，且 Hermes 已决定**不再自建 parser/AST**。

**新主链**：Hermes 消费 [`vos-language`](https://github.com/voml/vos-language) 产出的 service / operation projection artifact（JSON），按 artifact 中的稳定 identity 注册并路由 RPC 调用。

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

Hermes **不** vendor 本地镜像 crate；projection artifact 类型与校验直接依赖上游 `vos-types`（git `dev`，本地开发可用 `.cargo/config.toml` path patch）。

- **artifact 合同**：`ServiceProjectionArtifact` / `OperationProjectionArtifact`，与 `vos-language` serde 形状对齐（camelCase、`{start, end}` span、`deny_unknown_fields`），并复算校验 `svc_` / `op_` identity。
- **错误层**：`ArtifactError` 在 RPC 边界映射为 `RpcError`。

## `hermes-runtime` 提供

- `HermesServiceLoader` / `JsonServiceLoader`——artifact 解析与版本校验入口
- `HermesServiceRegistry` / `BTreeServiceRegistry`——service 与 operation 分开存储，重复 identity 注册期拒绝
- `HermesDispatcher` / `HermesOperationHandler`——按 `operation_id` 路由到用户 handler
- `HermesDispatchRequest` / `HermesDispatchResponse`——请求 / 响应 envelope
- `RpcError` / `RpcErrorCode`——稳定错误码族，wire 错误体为 `{"code": "...", "message": "..."}`
- `CallOptions` / `CancelToken` / `CallContext`——dispatch 生命周期：deadline、取消与在途并发上限

## `hermes-generator` 提供

- `generate` / `generate_from_json`——从 operation artifact 产出 `GenerationPlan`
- `hermes/operations.ts`——operation id 常量、参数 / 返回类型与 `createHermesClient(transport)`
- `hermes/server.ts`——按 `operation_id` 绑定 handler 的 `bindOperationHandlers()`
- `hermes/handlers.ts`——一次性业务模板，已存在则保留（`FileOwnership::Template`）
- `Diagnostic`——无法安全映射的类型降级为 `unknown` 并记录诊断，命名冲突直接拒绝

生成物不 import 任何 Hermes npm 包，只声明 `HermesTransport` 接口，由宿主运行时绑定传入。

## 接入 `vos-language`

Hermes 不编译依赖 `vos` crate；它只消费 `vos-language` 产出的 JSON artifact。字段命名、span 形状与 identity 规则以上游 `vos-types` 为准，镜像改动必须以上游真实 fixture 做解码测试。

## 验证

```bash
cargo test --workspace
```

57 个测试全部通过：18 个 projection 合同测试 + 29 个 runtime 测试 + 10 个 generator 测试。

## 设计约束

- **Hermes 不解析 VOS 源码**：VOS 文本 → ServiceContract artifact 在 `vos-language` 侧完成；Hermes 只消费 artifact。
- **1.0.0 前不保留旧 API 兼容层**：旧原型一旦无法编译就直接移除，不做 wrapper、alias、fallback 或双读双写。
- **错误自定**：Hermes 运行时错误模型独立于 `vos-language` 上游，避免跨仓耦合。
