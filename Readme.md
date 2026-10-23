# Hermes RPC

Hermes 是 VMZ 体系中的 RPC 协议宿主与服务消费端。

## 当前状态

**旧栈已冻结。** 2026-10-23 之前的 `vos-ast` / `vos-for-rust` / `vos-openapi` / `vos-cli` / `vos-faker` 原型已从 workspace 移除——它们依赖的 `diagnostic-quick 0.4.0` 与当前 `diagnostic` crate 不兼容，且 Hermes 已决定**不再自建 parser/AST**。

**新主链**：Hermes 消费 [`vos-language`](https://github.com/voml/vos-language) 产出的 `ServiceContract` artifact（JSON 描述），按 artifact 中的 service/method descriptor 注册并路由 RPC 调用。

## 工作区结构

```
hermes-rpc/
├── Cargo.toml                # workspace 入口，只含 projects/vos-core
├── projects/
│   └── vos-core/             # Hermes 核心类型 + ServiceContract 消费接口
└── projects/vos-*            # ← 冻结的旧原型，已移出 workspace，不再编译
```

## `vos-core` 提供

- **类型层**：`Project`、`Schema`、`Endpoint`、各种 constraint（`IntegerConstraint` / `DecimalConstraint` / `StringConstraint` …）——保留 VOS schema 的建模能力。
- **错误层**：`HermesError` / `QResult<T>` / `QError` —— 替代了已废弃的 `diagnostic-quick`。
- **Hermes 接口骨架**：
  - `HermesServiceLoader` —— 从 artifact JSON 加载 service 描述（内建 `JsonServiceLoader`）
  - `HermesServiceRegistry` —— 注册/查找 service（内建 `BTreeServiceRegistry`）
  - `HermesServiceContract` / `HermesMethodDescriptor` —— 占位的 ServiceContract 结构
  - `Validation<T>` —— 带诊断的结果类型

## 接入 `vos-language`

`HermesServiceContract` 是本地占位结构，真实类型由 `vos-language` 仓库导出。等上游 `vos` crate 暴露正式的 `ServiceContract` 后，只需在 `Cargo.toml` 添加：

```toml
[dependencies]
vos = { git = "https://github.com/voml/vos-language.git", branch = "dev" }
```

然后把本 crate 内的占位 trait 替换为 `use vos::ServiceContract`。当前上游 `oak-vos` 仍在 nightly 上，待其迁到 stable 后接通。

## 验证

```bash
cargo check --workspace
cargo test --workspace
```

7 个 mock 单元测试全部通过，覆盖 loader 解析、registry 注册/查找/去重、QError 构造器。

## 设计约束

- **Hermes 不解析 VOS 源码**：VOS 文本 → ServiceContract artifact 在 `vos-language` 侧完成；Hermes 只消费 artifact。
- **1.0.0 前不保留旧 API 兼容层**：旧原型一旦无法编译就直接移除，不做 wrapper、alias、fallback 或双读双写。
- **错误自定**：Hermes 运行时错误模型独立于 `vos-language` 上游，避免跨仓耦合。
