//! 占位 validator parser：旧栈基于 `vos-error` 的实现已冻结。
//! Hermes 不再自建 VOS parser，改消费 `vos-language` 的 ServiceContract artifact。

pub trait Validator {}

pub fn valid_email(_email: &str) -> bool {
    todo!("Hermes 不再自建 VOS parser，改消费 vos-language ServiceContract")
}
