//! HTTP/JSON-RPC 2.0 transport adapter for Hermes RPC.
//!
//! 本 crate 是 Hermes runtime 的第一个真实 transport adapter。职责：
//!
//! - JSON-RPC 2.0 wire 类型（`json_rpc` 模块）
//! - `HttpTransport`：持有 `Arc<HermesDispatcher>`，负责协议层适配
//! - 错误码映射：Hermes `RpcErrorCode` → JSON-RPC 2.0 标准码
//! - `axum::Router` 生成，挂载在 `/rpc`
//!
//! 骨架阶段只覆盖 unary 请求（单请求单响应）。server-stream、batch、
//! notification 等高级语义留待后续迭代。

#![deny(missing_docs)]

pub mod json_rpc;
pub mod transport;

pub use json_rpc::{JSONRPC_VERSION, JsonRpcError, JsonRpcErrorCode, JsonRpcRequest, JsonRpcResponse};
pub use transport::{HandleOutcome, HttpTransport, HttpTransportConfig, TransportError};
