use super::*;
use num_bigint::BigInt;
use rust_decimal::Decimal;
use voml_collection::Integer;

// Legacy feature: `json` was declared upstream but never wired in Cargo.toml.
// The `json` submodule below is kept for reference.
mod json;

/// schema 中可出现的对象值枚举（旧栈 AST 的 `Object` 镜像）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Object {
    /// 默认值（占位，表示未指定具体值）。
    Default,
    /// 布尔值。
    Boolean(bool),
    /// 整数值（任意精度）。
    Integer(Integer),
    /// 十进制值。
    Decimal(Decimal),
    /// 引用类型（引用另一个 schema 的名字）。
    Reference(String),
    /// 文本值（带 hint 的字符串）。
    Text(Text),
    /// 有序列表值。
    List(List),
    /// 字典值。
    Dict(Dict),
}

impl From<&str> for Object {
    fn from(value: &str) -> Self {
        Object::text(value, "")
    }
}

impl From<&String> for Object {
    fn from(value: &String) -> Self {
        Object::text(value, "")
    }
}

impl From<String> for Object {
    fn from(value: String) -> Self {
        Object::text(value, "")
    }
}

impl Object {
    /// 构造一个 `Object::Text`。
    pub fn text(text: impl Into<String>, hint: impl Into<String>) -> Self {
        Object::Text(Text { hint: hint.into(), text: text.into() })
    }
    /// 构造一个 `Object::Integer`。
    pub fn integer(number: impl Into<BigInt>, hint: impl Into<String>) -> Self {
        Object::Integer(Integer { hint: hint.into(), value: number.into() })
    }
}
