//! 占位 parser：旧栈基于 `vos-ast` 的实现已冻结。
//! Hermes 不再自建 VOS parser，改消费 `vos-language` 的 ServiceContract artifact。

use crate::Validation;
use crate::schema::Schema;

impl Schema {
    pub fn parse(_input: &str) -> Validation<Self> {
        todo!("Hermes 不再自建 VOS parser，改消费 vos-language ServiceContract")
    }
}
