//! VOS canonical type → Hermes wire codec。
//!
//! codec 只声明某种 VOS 合同类型在 wire 上的**表达**，并据此校验 / 解码调用方
//! 参数与 handler 返回值。它不改变 VOS 已经确定的类型语义：`i64` 仍然是 64 位
//! 有符号整数，只是 wire 上用 JSON string 承载以避免 JavaScript number 精度丢失。
//!
//! 未在 descriptor 表里声明的 `Named`（包括 `json` / `value` / `any`）、未识别的
//! `Generic` 以及无法安全表达的形状一律返回 `unsupported-codec`，绝不降级成
//! `unknown` 或 `any`。

use serde_json::Value;
use vos_types::{CanonicalType, CanonicalTypeArgument, OperationProjection};

use crate::error::{RpcError, RpcErrorCode, RpcResult};

/// 一个 VOS 合同类型的 wire 表达。
///
/// 标量按 Hermes 的临时类型表声明：`i8`…`u32` 走 JSON number，`i64` / `u64` /
/// `int` / `decimal` / `uuid` / 时间类 / `bytes` 走 JSON string，避免 JS number
/// 精度丢失。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireCodec {
    /// JSON boolean（`bool` / `boolean`）。
    Bool,
    /// JSON string（`utf8` / `string` / `text` / `str`）。
    Utf8,
    /// JSON integer number，限定在 `[min, max]` 区间（`i8` / `i16` / `i32` / `u8` / `u16` / `u32`）。
    Integer {
        /// 下界（含）。
        min: i64,
        /// 上界（含）。
        max: i64,
    },
    /// JSON number（`f32` / `f64` / `float` / `number`）。
    Float,
    /// JSON string 承载的 64 位有符号整数（`i64`）。
    Int64,
    /// JSON string 承载的 64 位无符号整数（`u64`）。
    Uint64,
    /// JSON string 承载的任意精度整数（`int` / `integer` / `bigint`）。
    IntegerString,
    /// JSON string 承载的十进制数（`decimal`）。
    Decimal,
    /// JSON string 承载的 UUID（`uuid`）。
    Uuid,
    /// JSON string 承载的日期（`date`）。
    Date,
    /// JSON string 承载的时间（`time`）。
    Time,
    /// JSON string 承载的日期时间（`datetime` / `timestamp`）。
    DateTime,
    /// JSON string 承载的 UTC 日期时间（`datetimeUtc`）。
    DateTimeUtc,
    /// JSON string 承载的二进制数据（`bytes` / `binary`）。
    Bytes,
    /// `null` 或内部 codec 的值。
    Optional(Box<WireCodec>),
    /// 内部 codec 的值组成的 JSON array。
    List(Box<WireCodec>),
}

impl WireCodec {
    /// 稳定的人类可读名称，用于诊断与测试断言。
    pub fn label(&self) -> String {
        match self {
            Self::Bool => "bool".to_string(),
            Self::Utf8 => "utf8".to_string(),
            Self::Integer { min, max } => format!("integer[{min},{max}]"),
            Self::Float => "float".to_string(),
            Self::Int64 => "i64".to_string(),
            Self::Uint64 => "u64".to_string(),
            Self::IntegerString => "int".to_string(),
            Self::Decimal => "decimal".to_string(),
            Self::Uuid => "uuid".to_string(),
            Self::Date => "date".to_string(),
            Self::Time => "time".to_string(),
            Self::DateTime => "datetime".to_string(),
            Self::DateTimeUtc => "datetimeUtc".to_string(),
            Self::Bytes => "bytes".to_string(),
            Self::Optional(inner) => format!("Optional<{}>", inner.label()),
            Self::List(inner) => format!("List<{}>", inner.label()),
        }
    }
}

/// 把 canonical type 解析成 wire codec；无法安全表达时返回 `unsupported-codec`。
///
/// `Reference<T>` 被视为其内部类型的透明包装：引用是 identity 层的语义，wire
/// 上只承载被引用的键值。
pub fn codec_for(ty: &CanonicalType) -> RpcResult<WireCodec> {
    match ty {
        CanonicalType::Named(path) => codec_for_named(path),
        CanonicalType::Reference(inner) => codec_for(inner),
        CanonicalType::Optional(inner) => Ok(WireCodec::Optional(Box::new(codec_for(inner)?))),
        CanonicalType::List(inner) => Ok(WireCodec::List(Box::new(codec_for(inner)?))),
        CanonicalType::Generic { path, arguments } => codec_for_generic(path, arguments),
    }
}

fn codec_for_named(path: &[String]) -> RpcResult<WireCodec> {
    let name = path.last().map(String::as_str).unwrap_or_default();
    let codec = match name {
        "bool" | "boolean" => WireCodec::Bool,
        "utf8" | "string" | "text" | "str" => WireCodec::Utf8,
        "i8" => WireCodec::Integer { min: i64::from(i8::MIN), max: i64::from(i8::MAX) },
        "i16" => WireCodec::Integer { min: i64::from(i16::MIN), max: i64::from(i16::MAX) },
        "i32" => WireCodec::Integer { min: i64::from(i32::MIN), max: i64::from(i32::MAX) },
        "u8" => WireCodec::Integer { min: 0, max: i64::from(u8::MAX) },
        "u16" => WireCodec::Integer { min: 0, max: i64::from(u16::MAX) },
        "u32" => WireCodec::Integer { min: 0, max: i64::from(u32::MAX) },
        "f32" | "f64" | "float" | "number" => WireCodec::Float,
        "i64" => WireCodec::Int64,
        "u64" => WireCodec::Uint64,
        "int" | "integer" | "bigint" => WireCodec::IntegerString,
        "decimal" => WireCodec::Decimal,
        "uuid" => WireCodec::Uuid,
        "date" => WireCodec::Date,
        "time" => WireCodec::Time,
        "datetime" | "timestamp" => WireCodec::DateTime,
        "datetimeUtc" => WireCodec::DateTimeUtc,
        "bytes" | "binary" => WireCodec::Bytes,
        _ => return Err(unsupported(display_path(path))),
    };
    Ok(codec)
}

fn codec_for_generic(path: &[String], arguments: &[CanonicalTypeArgument]) -> RpcResult<WireCodec> {
    let name = path.last().map(|segment| segment.to_ascii_lowercase()).unwrap_or_default();
    let ty = CanonicalType::Generic { path: path.to_vec(), arguments: arguments.to_vec() };

    let single_type_argument = match arguments {
        [CanonicalTypeArgument::Type(inner)] => Some(inner),
        _ => None,
    };

    match (name.as_str(), single_type_argument) {
        ("list" | "array" | "vec", Some(inner)) => Ok(WireCodec::List(Box::new(codec_for(inner)?))),
        ("optional" | "option", Some(inner)) => Ok(WireCodec::Optional(Box::new(codec_for(inner)?))),
        _ => Err(unsupported(ty_label(&ty))),
    }
}

fn unsupported(type_label: String) -> RpcError {
    RpcError::new(RpcErrorCode::UnsupportedCodec, format!("unsupported codec for type `{type_label}`"))
}

fn ty_label(ty: &CanonicalType) -> String {
    match ty {
        CanonicalType::Named(path) => display_path(path),
        CanonicalType::Reference(inner) => format!("Reference<{}>", ty_label(inner)),
        CanonicalType::Optional(inner) => format!("Optional<{}>", ty_label(inner)),
        CanonicalType::List(inner) => format!("List<{}>", ty_label(inner)),
        CanonicalType::Generic { path, arguments } => {
            let arguments: Vec<String> = arguments
                .iter()
                .map(|argument| match argument {
                    CanonicalTypeArgument::Type(ty) => ty_label(ty),
                    CanonicalTypeArgument::Literal(text) => format!("literal `{text}`"),
                })
                .collect();
            format!("{}<{}>", display_path(path), arguments.join(", "))
        }
    }
}

fn display_path(path: &[String]) -> String {
    if path.is_empty() { "<empty path>".to_string() } else { path.join(".") }
}

/// 按 operation 的 `parameters` 解码调用方 wire 参数对象。
///
/// 返回只包含已声明参数的新对象。缺失参数、多余参数与 codec 不符都返回
/// `invalid-request`；参数类型没有 descriptor 时返回 `unsupported-codec`。
pub fn decode_params(projection: &OperationProjection, params: &Value) -> RpcResult<Value> {
    let object = params.as_object().ok_or_else(|| {
        RpcError::new(RpcErrorCode::InvalidRequest, format!("params must be a JSON object, found {}", json_kind(params)))
    })?;

    for key in object.keys() {
        if !projection.parameters.iter().any(|parameter| parameter.name == *key) {
            return Err(RpcError::new(RpcErrorCode::InvalidRequest, format!("unknown parameter `{key}`")));
        }
    }

    let mut decoded = serde_json::Map::with_capacity(projection.parameters.len());
    for parameter in &projection.parameters {
        let value = object.get(&parameter.name).ok_or_else(|| {
            RpcError::new(RpcErrorCode::InvalidRequest, format!("missing parameter `{}`", parameter.name))
        })?;
        let codec = codec_for(&parameter.type_expr)?;
        let path = format!("parameter `{}`", parameter.name);
        decoded.insert(parameter.name.clone(), check_value(&codec, value, &path, RpcErrorCode::InvalidRequest)?);
    }
    Ok(Value::Object(decoded))
}

/// 按 operation 的 `return_type` 编码 handler 返回值。
///
/// `return_type` 为 `None` 时不做额外约束，原样返回。声明了返回类型但 wire
/// 形状不符时返回 `handler-failed`；返回类型没有 descriptor 时返回
/// `unsupported-codec`。
pub fn encode_result(projection: &OperationProjection, value: &Value) -> RpcResult<Value> {
    match &projection.return_type {
        None => Ok(value.clone()),
        Some(ty) => {
            let codec = codec_for(ty)?;
            check_value(&codec, value, "return value", RpcErrorCode::HandlerFailed)
        }
    }
}

/// 按单个 codec 解码 wire 值（调用方参数方向），形状不符返回 `invalid-request`。
pub fn decode_value(codec: &WireCodec, value: &Value) -> RpcResult<Value> {
    check_value(codec, value, "value", RpcErrorCode::InvalidRequest)
}

/// 按单个 codec 编码合同值（handler 返回方向），形状不符返回 `handler-failed`。
pub fn encode_value(codec: &WireCodec, value: &Value) -> RpcResult<Value> {
    check_value(codec, value, "value", RpcErrorCode::HandlerFailed)
}

fn check_value(codec: &WireCodec, value: &Value, path: &str, code: RpcErrorCode) -> RpcResult<Value> {
    match codec {
        WireCodec::Optional(inner) => {
            if value.is_null() {
                return Ok(Value::Null);
            }
            check_value(inner, value, path, code)
        }
        WireCodec::List(inner) => {
            let array = value.as_array().ok_or_else(|| mismatch(codec, path, value, code))?;
            let mut items = Vec::with_capacity(array.len());
            for (index, item) in array.iter().enumerate() {
                items.push(check_value(inner, item, &format!("{path}[{index}]"), code)?);
            }
            Ok(Value::Array(items))
        }
        WireCodec::Bool => {
            if value.as_bool().is_none() {
                return Err(mismatch(codec, path, value, code));
            }
            Ok(value.clone())
        }
        WireCodec::Utf8 => expect_string(codec, value, path, code),
        WireCodec::Integer { min, max } => {
            let number = value.as_i64().ok_or_else(|| mismatch(codec, path, value, code))?;
            if number < *min || number > *max {
                return Err(RpcError::new(
                    code,
                    format!("{path} is out of range for codec {}: {number} not in [{min}, {max}]", codec.label()),
                ));
            }
            Ok(value.clone())
        }
        WireCodec::Float => {
            if value.as_f64().is_none() {
                return Err(mismatch(codec, path, value, code));
            }
            Ok(value.clone())
        }
        WireCodec::Int64 => {
            let text = expect_str(codec, value, path, code)?;
            text.parse::<i64>().map_err(|_| {
                RpcError::new(code, format!("{path} must be a decimal i64 string, found `{text}`"))
            })?;
            Ok(value.clone())
        }
        WireCodec::Uint64 => {
            let text = expect_str(codec, value, path, code)?;
            text.parse::<u64>().map_err(|_| {
                RpcError::new(code, format!("{path} must be a decimal u64 string, found `{text}`"))
            })?;
            Ok(value.clone())
        }
        WireCodec::IntegerString => {
            let text = expect_str(codec, value, path, code)?;
            if !is_integer_text(text) {
                return Err(RpcError::new(code, format!("{path} must be a decimal integer string, found `{text}`")));
            }
            Ok(value.clone())
        }
        WireCodec::Decimal => {
            let text = expect_str(codec, value, path, code)?;
            if !is_decimal_text(text) {
                return Err(RpcError::new(code, format!("{path} must be a decimal string, found `{text}`")));
            }
            Ok(value.clone())
        }
        WireCodec::Uuid
        | WireCodec::Date
        | WireCodec::Time
        | WireCodec::DateTime
        | WireCodec::DateTimeUtc
        | WireCodec::Bytes => expect_string(codec, value, path, code),
    }
}

fn expect_str<'a>(codec: &WireCodec, value: &'a Value, path: &str, code: RpcErrorCode) -> RpcResult<&'a str> {
    value.as_str().ok_or_else(|| mismatch(codec, path, value, code))
}

fn expect_string(codec: &WireCodec, value: &Value, path: &str, code: RpcErrorCode) -> RpcResult<Value> {
    expect_str(codec, value, path, code)?;
    Ok(value.clone())
}

fn mismatch(codec: &WireCodec, path: &str, value: &Value, code: RpcErrorCode) -> RpcError {
    RpcError::new(
        code,
        format!(
            "{path} must be JSON {}, found {} (codec {})",
            expected_shape(codec),
            json_kind(value),
            codec.label()
        ),
    )
}

fn expected_shape(codec: &WireCodec) -> &'static str {
    match codec {
        WireCodec::Bool => "boolean",
        WireCodec::Integer { .. } => "integer number",
        WireCodec::Float => "number",
        WireCodec::List(_) => "array",
        WireCodec::Optional(_) => "value",
        WireCodec::Utf8
        | WireCodec::Int64
        | WireCodec::Uint64
        | WireCodec::IntegerString
        | WireCodec::Decimal
        | WireCodec::Uuid
        | WireCodec::Date
        | WireCodec::Time
        | WireCodec::DateTime
        | WireCodec::DateTimeUtc
        | WireCodec::Bytes => "string",
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn is_integer_text(text: &str) -> bool {
    let digits = strip_sign(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_decimal_text(text: &str) -> bool {
    let unsigned = strip_sign(text);
    let mut seen_digit = false;
    let mut seen_dot = false;
    for byte in unsigned.bytes() {
        match byte {
            b'0'..=b'9' => seen_digit = true,
            b'.' if !seen_dot => seen_dot = true,
            _ => return false,
        }
    }
    seen_digit
}

fn strip_sign(text: &str) -> &str {
    text.strip_prefix('-').or_else(|| text.strip_prefix('+')).unwrap_or(text)
}
