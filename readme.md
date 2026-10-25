# Hermes RPC

Hermes 是 VMZ 的 **RPC 合同消费者、AOT 生成器与协议服务宿主**。它消费 [`vos-language`](https://github.com/voml/vos-language) 产出的 frozen contract artifact（`vos/hermes.contract/v0`），按其中的 `ServiceContract` / `ServiceMethodContract`、`MethodId` 与 `OperationId` 注册、路由并生成 RPC 代码。

## 设计真相源

架构、阶段门与验收标准以 **VMZ 根设计仓的 Hermes 架构文档** 为准。本仓文档只描述目录、入口与验证方式，不重复、不覆盖、也不私有化设计决策；两者冲突时以设计真相源为准。

## 给 agent 的阅读顺序

按顺序读，约 5 分钟即可建立全局认识，避免逐文件扫描：

1. **本文件** —— 定位、目录与边界。
2. **`projects/hermes.rs/crates/hermes-runtime/src/lib.rs`** —— 领域核心：artifact loader、`ContractRegistry`、`(service, method)` 路由、dispatch、codec、lifecycle、结构化错误。RPC 语义都在这里。
3. **`projects/hermes.rs/crates/hermes-generator/src/lib.rs`** —— AOT 生成入口 `generate_service`，产物固定落在 `hermes/service/`。
4. **`hermes-napi/src/lib.rs`** 与 **`hermes-wasm/src/lib.rs`** —— 两个宿主边界：Node 多协议 server 宿主、浏览器 WASM。
5. **TypeScript 侧** —— `packages/hermes`（Node N-API 客户端）、`packages/hermes-language`（浏览器封装）、`platforms/hermes-unknown-wasm32`（wasm 绑定产物）。
6. 改代码前先确认下方「硬边界」。

## 工作区结构

```
hermes-rpc/
├── Cargo.toml                          # workspace：仅 4 个 Rust crate
├── projects/
│   ├── hermes.rs/crates/
│   │   ├── hermes-runtime/             # RPC 领域运行时
│   │   ├── hermes-generator/           # AOT 生成器
│   │   ├── hermes-napi/                # N-API 宿主（server 侧）
│   │   └── hermes-wasm/                # WASM 宿主（Web 侧）
│   └── hermes.ts/
│       ├── packages/                   # @yydb/hermes、@yydb/hermes-language、skills
│       └── platforms/                  # 各平台 N-API addon + unknown-wasm32
```

## 硬边界（改代码前必读）

- **Hermes 不解析 VOS 源码。** VOS 文本 → contract artifact 在 `vos-language` 侧完成；本仓只消费 artifact，不读 `body` 文本、不自建 method 绑定表。
- **不提供官方 client runtime。** client 由 `ServiceContract` AOT 生成普通源码，transport 由目标项目用已有框架实现。
- **不 vendor 本地类型镜像。** 合同类型、identity、fingerprint、canonical type 直接依赖上游 `vos-types`（git `dev`）；本仓只做 serde mirror 与一致性校验。
- **server 统一走 N-API 多协议宿主。** Hermes 不自建通用 HTTP server，部署与 transport 具体实现归宿主。
- **Web 只做 Web。** `@yydb/hermes-language` 只基于 `@yydb/hermes-unknown-wasm32`，不依赖 `@yydb/hermes`；生成物、包、WASM 三层禁止反向依赖。
- **1.0.0 前不保留兼容层。** 旧 `vos-ast` / `vos-for-rust` / `vos-openapi` / `vos-cli` / `vos-faker` 已移出 workspace，不恢复、不 wrapper。

## 文档约定

本仓只保留：本入口 README、各 npm 包的发布 README（`package.json` 的 `files` 依赖它们）、以及 `hermes-skills` 的 SKILL/README。新增设计说明一律写入根设计仓，不在本仓创建 `docs/`、交接文档或第二份架构说明。

## 验证

```bash
cargo test --workspace
```

2026-10-25 全绿：85 个测试（runtime 合同 / registry / dispatch / codec、generator、wasm binding）。
