//! HTTP/JSON-RPC 2.0 transport adapter for Hermes RPC.
//!
//! 本 crate 是 Hermes runtime 的第一个真实 transport adapter。职责：
//!
//! - JSON-RPC 2.0 wire 类型（`json_rpc` 模块）
//! - `HttpTransport`：持有 `Arc<HermesDispatcher>`，负责协议层适配
//! - 错误码映射：[`transport::map_rpc_error_to_jsonrpc`] 是 Hermes `RpcErrorCode`
//!   → JSON-RPC 2.0 错误体的唯一映射表
//! - 生命周期桥接：wire `deadline` 成员与客户断连折算成 runtime 的
//!   `CallOptions`（deadline + `CancelToken`），不在 transport 内另造取消状态机
//! - `axum::Router` 生成，挂载在 `/rpc`

#![deny(missing_docs)]

pub mod json_rpc;
pub mod transport;

pub use json_rpc::{JSONRPC_VERSION, JsonRpcError, JsonRpcErrorCode, JsonRpcRequest, JsonRpcResponse};
pub use transport::{HandleOutcome, HttpTransport, HttpTransportConfig, TransportError, map_rpc_error_to_jsonrpc};
