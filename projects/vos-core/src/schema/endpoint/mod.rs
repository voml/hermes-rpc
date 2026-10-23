use super::*;

/// 对外暴露的 HTTP/RPC endpoint 占位。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Endpoint {
    /// 请求体类型名。
    pub request: String,
    /// 响应体类型名。
    pub response: String,
    /// HTTP 侧描述（旧栈残留）。
    pub get: Request,
    /// 文档说明。
    pub document: Document,
}


/// HTTP request 占位类型。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Request {
    open: String,
}

impl Endpoint {
    /// HTTP GET endpoint 占位构造器（当前为空实现）。
    pub fn http_get() {}
}
