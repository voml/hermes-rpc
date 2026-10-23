use super::*;
use num_bigint::BigInt;
use rust_decimal::Decimal;
use voml_collection::Integer;

// Legacy feature: `json` was declared upstream but never wired in Cargo.toml.
// The `json` submodule below is kept for reference.
mod json;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Object {
    Default,
    Boolean(bool),
    Integer(Integer),
    Decimal(Decimal),
    Reference(String),
    Text(Text),
    List(List),
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
    pub fn text(text: impl Into<String>, hint: impl Into<String>) -> Self {
        Object::Text(Text { hint: hint.into(), text: text.into() })
    }
    pub fn integer(number: impl Into<BigInt>, hint: impl Into<String>) -> Self {
        Object::Integer(Integer { hint: hint.into(), value: number.into() })
    }
}
