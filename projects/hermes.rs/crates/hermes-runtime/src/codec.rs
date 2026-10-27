//! VOS canonical type → Hermes wire codec。
//!
//! codec 只声明合同类型在 wire 上的**表达**，不改变 `vos-types` 已经确定的语义。
//! 标量按 `CanonicalScalar` 的 19 个 typed 变体逐一映射：`utf8` 与 `utf16` 保持
//! 区分，`decimal` 仍是十进制字符串，`i64` / `u64` 用字符串避免 JavaScript
//! number 精度丢失。
//!
//! 用户类型（`ResolvedCanonicalType::User`）在合同里只有 canonical path 与
//! durable `type_id`，没有字段布局，因此 wire 上按 JSON object 结构校验，不做
//! 字段级推断。未声明的 generic 与任何其他形状返回 `unsupported-codec`，绝不
//! 降级成 `any` 或默认值。

use serde_json::Value;
use vos_types::{CanonicalScalar, OperationContract, ResolvedCanonicalType, ResolvedCanonicalTypeArgument};

use crate::error::{RpcError, RpcErrorCode, RpcResult};

/// 一个 VOS 合同类型的 wire 表达。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireCodec {
    /// JSON boolean（`bool`）。
    Bool,
    /// JSON integer number，限定在 `[min, max]` 区间。
    Integer {
        /// 下界（含）。
        min: i64,
        /// 上界（含）。
        max: i64,
    },
    /// JSON number（`f32` / `f64`）。
    Float,
    /// JSON string 承载的 64 位有符号整数（`i64`）。
    Int64,
    /// JSON string 承载的 64 位无符号整数（`u64`）。
    Uint64,
    /// JSON string 承载的十进制数（`decimal`）。
    Decimal,
    /// JSON string（`utf8`）。
    Utf8,
    /// JSON string（`utf16`，与 `utf8` 不合并）。
    Utf16,
    /// JSON string 承载的 UUID（`uuid`）。
    Uuid,
    /// JSON string 承载的日期（`date`）。
    Date,
    /// JSON string 承载的时间（`time`）。
    Time,
    /// JSON string 承载的 UTC 日期时间（`datetime`）。
    DateTime,
    /// JSON array 承载的二进制数据（`bytes`），每个元素是 `u8`（0–255）。
    Bytes,
    /// JSON object，承载用户类型（合同不携带字段布局）。
    Object {
        /// 用户类型的 canonical path 标签。
        path: String,
    },
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
            Self::Integer { min, max } => format!("integer[{min},{max}]"),
            Self::Float => "float".to_string(),
            Self::Int64 => "i64".to_string(),
            Self::Uint64 => "u64".to_string(),
            Self::Decimal => "decimal".to_string(),
            Self::Utf8 => "utf8".to_string(),
            Self::Utf16 => "utf16".to_string(),
            Self::Uuid => "uuid".to_string(),
            Self::Date => "date".to_string(),
            Self::Time => "time".to_string(),
            Self::DateTime => "datetime".to_string(),
            Self::Bytes => "bytes".to_string(),
            Self::Object { path } => format!("object:{path}"),
            Self::Optional(inner) => format!("Optional<{}>", inner.label()),
            Self::List(inner) => format!("List<{}>", inner.label()),
        }
    }
}

/// 把 canonical type 解析成 wire codec；无法安全表达时返回 `unsupported-codec`。
///
/// `Reference<T>` 是 identity 层的透明包装，wire 上只承载被引用的值。
pub fn codec_for(ty: &ResolvedCanonicalType) -> RpcResult<WireCodec> {
    match ty {
        ResolvedCanonicalType::Scalar(scalar) => Ok(scalar_codec(*scalar)),
        ResolvedCanonicalType::User { path, type_id } => {
            Ok(WireCodec::Object { path: format!("{}#{type_id}", display_path(path)) })
        }
        ResolvedCanonicalType::Reference(inner) => codec_for(inner),
        ResolvedCanonicalType::Optional(inner) => Ok(WireCodec::Optional(Box::new(codec_for(inner)?))),
        ResolvedCanonicalType::List(inner) => Ok(WireCodec::List(Box::new(codec_for(inner)?))),
        ResolvedCanonicalType::Generic { path, arguments } => codec_for_generic(path, arguments),
    }
}

fn scalar_codec(scalar: CanonicalScalar) -> WireCodec {
    match scalar {
        CanonicalScalar::I8 => WireCodec::Integer { min: i64::from(i8::MIN), max: i64::from(i8::MAX) },
        CanonicalScalar::I16 => WireCodec::Integer { min: i64::from(i16::MIN), max: i64::from(i16::MAX) },
        CanonicalScalar::I32 => WireCodec::Integer { min: i64::from(i32::MIN), max: i64::from(i32::MAX) },
        CanonicalScalar::U8 => WireCodec::Integer { min: 0, max: i64::from(u8::MAX) },
        CanonicalScalar::U16 => WireCodec::Integer { min: 0, max: i64::from(u16::MAX) },
        CanonicalScalar::U32 => WireCodec::Integer { min: 0, max: i64::from(u32::MAX) },
        CanonicalScalar::I64 => WireCodec::Int64,
        CanonicalScalar::U64 => WireCodec::Uint64,
        CanonicalScalar::F32 | CanonicalScalar::F64 => WireCodec::Float,
        CanonicalScalar::Bool => WireCodec::Bool,
        CanonicalScalar::Utf8 => WireCodec::Utf8,
        CanonicalScalar::Utf16 => WireCodec::Utf16,
        CanonicalScalar::Uuid => WireCodec::Uuid,
        CanonicalScalar::Decimal => WireCodec::Decimal,
        CanonicalScalar::Date => WireCodec::Date,
        CanonicalScalar::Time => WireCodec::Time,
        CanonicalScalar::DateTime => WireCodec::DateTime,
        CanonicalScalar::Bytes => WireCodec::Bytes,
    }
}

fn codec_for_generic(path: &[String], arguments: &[ResolvedCanonicalTypeArgument]) -> RpcResult<WireCodec> {
    let name = path.last().map(|segment| segment.to_ascii_lowercase()).unwrap_or_default();
    let single_type_argument = match arguments {
        [ResolvedCanonicalTypeArgument::Type(inner)] => Some(inner),
        _ => None,
    };

    match (name.as_str(), single_type_argument) {
        ("list" | "array" | "vec", Some(inner)) => Ok(WireCodec::List(Box::new(codec_for(inner)?))),
        ("optional" | "option", Some(inner)) => Ok(WireCodec::Optional(Box::new(codec_for(inner)?))),
        _ => Err(RpcError::new(
            RpcErrorCode::UnsupportedCodec,
            format!("unsupported codec for type `{}`", ty_label(path, arguments)),
        )),
    }
}

/// 按 operation 的 `parameters` 解码调用方 wire 参数对象。
///
/// 返回只包含已声明参数的新对象。缺失参数、多余参数与 codec 不符都返回
/// `invalid-request`；参数类型没有 codec 时返回 `unsupported-codec`。
pub fn decode_params(operation: &OperationContract, params: &Value) -> RpcResult<Value> {
    let object = params.as_object().ok_or_else(|| {
        RpcError::new(RpcErrorCode::InvalidRequest, format!("params must be a JSON object, found {}", json_kind(params)))
    })?;

    for key in object.keys() {
        if !operation.parameters.iter().any(|parameter| parameter.name == *key) {
            return Err(RpcError::new(RpcErrorCode::InvalidRequest, format!("unknown parameter `{key}`")));
        }
    }

    let mut decoded = serde_json::Map::with_capacity(operation.parameters.len());
    for parameter in &operation.parameters {
        let value = object
            .get(&parameter.name)
            .ok_or_else(|| RpcError::new(RpcErrorCode::InvalidRequest, format!("missing parameter `{}`", parameter.name)))?;
        let codec = codec_for(&parameter.canonical_type)?;
        let path = format!("parameter `{}`", parameter.name);
        decoded.insert(parameter.name.clone(), check_value(&codec, value, &path, RpcErrorCode::InvalidRequest)?);
    }
    Ok(Value::Object(decoded))
}

/// 按单个 codec 解码 wire 值（调用方参数方向），形状不符返回 `invalid-request`。
pub fn decode_value(codec: &WireCodec, value: &Value) -> RpcResult<Value> {
    check_value(codec, value, "value", RpcErrorCode::InvalidRequest)
}

/// 按单个 codec 编码合同值（handler 返回方向），形状不符返回 `handler-failed`。
pub fn encode_value(codec: &WireCodec, value: &Value) -> RpcResult<Value> {
    check_value(codec, value, "value", RpcErrorCode::HandlerFailed)
}

/// 按给定错误码校验一个 wire 值是否匹配 codec。
pub fn check_value(codec: &WireCodec, value: &Value, path: &str, code: RpcErrorCode) -> RpcResult<Value> {
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
        WireCodec::Utf8
        | WireCodec::Utf16
        | WireCodec::Uuid
        | WireCodec::Date
        | WireCodec::Time
        | WireCodec::DateTime => expect_string(codec, value, path, code),
        WireCodec::Bytes => {
            let array = value.as_array().ok_or_else(|| mismatch(codec, path, value, code))?;
            let mut items = Vec::with_capacity(array.len());
            for (index, item) in array.iter().enumerate() {
                let byte = item.as_u64().ok_or_else(|| {
                    RpcError::new(
                        code,
                        format!("{path}[{index}] must be a u8 integer, found {}", json_kind(item)),
                    )
                })?;
                if byte > 255 {
                    return Err(RpcError::new(
                        code,
                        format!("{path}[{index}] must be in 0..=255, found {byte}"),
                    ));
                }
                items.push(Value::Number(serde_json::Number::from(byte as u8)));
            }
            Ok(Value::Array(items))
        }
        WireCodec::Object { .. } => {
            if !value.is_object() {
                return Err(mismatch(codec, path, value, code));
            }
            Ok(value.clone())
        }
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
            text.parse::<i64>()
                .map_err(|_| RpcError::new(code, format!("{path} must be a decimal i64 string, found `{text}`")))?;
            Ok(value.clone())
        }
        WireCodec::Uint64 => {
            let text = expect_str(codec, value, path, code)?;
            text.parse::<u64>()
                .map_err(|_| RpcError::new(code, format!("{path} must be a decimal u64 string, found `{text}`")))?;
            Ok(value.clone())
        }
        WireCodec::Decimal => {
            let text = expect_str(codec, value, path, code)?;
            if !is_decimal_text(text) {
                return Err(RpcError::new(code, format!("{path} must be a decimal string, found `{text}`")));
            }
            Ok(value.clone())
        }
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
        format!("{path} must be JSON {}, found {} (codec {})", expected_shape(codec), json_kind(value), codec.label()),
    )
}

fn expected_shape(codec: &WireCodec) -> &'static str {
    match codec {
        WireCodec::Bool => "boolean",
        WireCodec::Integer { .. } => "integer number",
        WireCodec::Float => "number",
        WireCodec::List(_) | WireCodec::Bytes => "array",
        WireCodec::Optional(_) => "value",
        WireCodec::Object { .. } => "object",
        WireCodec::Utf8
        | WireCodec::Utf16
        | WireCodec::Uuid
        | WireCodec::Date
        | WireCodec::Time
        | WireCodec::DateTime
        | WireCodec::Int64
        | WireCodec::Uint64
        | WireCodec::Decimal => "string",
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

fn is_decimal_text(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
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

fn ty_label(path: &[String], arguments: &[ResolvedCanonicalTypeArgument]) -> String {
    let arguments = arguments
        .iter()
        .map(|argument| match argument {
            ResolvedCanonicalTypeArgument::Type(ty) => codec_type_label(ty),
            ResolvedCanonicalTypeArgument::Literal(text) => format!("#{text}"),
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{}<{arguments}>", display_path(path))
}

fn codec_type_label(ty: &ResolvedCanonicalType) -> String {
    match ty {
        ResolvedCanonicalType::Scalar(scalar) => scalar.as_str().to_string(),
        ResolvedCanonicalType::User { path, type_id } => format!("{}#{type_id}", display_path(path)),
        ResolvedCanonicalType::Reference(inner) => format!("&{}", codec_type_label(inner)),
        ResolvedCanonicalType::Optional(inner) => format!("{}?", codec_type_label(inner)),
        ResolvedCanonicalType::List(inner) => format!("[{}]", codec_type_label(inner)),
        ResolvedCanonicalType::Generic { path, arguments } => ty_label(path, arguments),
    }
}

fn display_path(path: &[String]) -> String {
    if path.is_empty() { "<empty path>".to_string() } else { path.join("::") }
}
