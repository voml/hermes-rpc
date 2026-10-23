//! 占位 validator 模块：旧栈基于 `vos-error` 的实现已冻结。
//! Hermes 不再自建 VOS parser，改消费 `vos-language` 的 ServiceContract artifact。

/// 旧栈 Validator trait 占位。
pub trait Validator {}

/// 旧栈 email 校验占位（永远 panic）。
pub fn valid_email(_email: &str) -> bool {
    todo!("Hermes 不再自建 VOS parser，改消费 vos-language ServiceContract")
}
