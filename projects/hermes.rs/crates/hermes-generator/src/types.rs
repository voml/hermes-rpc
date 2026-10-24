//! VOS canonical type → TypeScript 类型表达式。
//!
//! 这是**临时映射表**：VOS 的 decimal / uuid / date / bytes 等合同类型的 wire
//! 表达要由 codec descriptor 显式声明（见架构文档 §4）。在 descriptor 落地前，
//! 这里只做保守映射：无法安全对应到 JS 原语的名字一律降级为 `unknown` 并记录
//! 诊断，绝不臆造一个更窄的类型。

use vos_types::{CanonicalType, CanonicalTypeArgument};

/// 一个 canonical type 的解析结果。
pub(crate) struct ResolvedType {
    /// TS 类型表达式。
    pub(crate) ts: String,
    /// 无法解析的名字（用于生成诊断）。
    pub(crate) unresolved: Vec<String>,
}

/// 无声明返回类型时使用的 TS 类型。
pub(crate) const NO_RETURN_TYPE: &str = "void";

const UNKNOWN: &str = "unknown";

/// 解析一个 canonical type。
pub(crate) fn resolve(ty: &CanonicalType) -> ResolvedType {
    match ty {
        CanonicalType::Named(path) => resolve_named(path),
        CanonicalType::Reference(inner) => resolve(inner),
        CanonicalType::Optional(inner) => {
            let inner = resolve(inner);
            ResolvedType { ts: format!("{} | null", parenthesize(&inner.ts)), unresolved: inner.unresolved }
        }
        CanonicalType::List(inner) => {
            let inner = resolve(inner);
            ResolvedType { ts: format!("{}[]", parenthesize(&inner.ts)), unresolved: inner.unresolved }
        }
        CanonicalType::Generic { path, arguments } => resolve_generic(path, arguments),
    }
}

fn resolve_named(path: &[String]) -> ResolvedType {
    let name = path.last().map(String::as_str).unwrap_or_default();
    if let Some(ts) = builtin(name) {
        return ResolvedType { ts: ts.to_string(), unresolved: Vec::new() };
    }
    ResolvedType { ts: UNKNOWN.to_string(), unresolved: vec![display_path(path)] }
}

fn resolve_generic(path: &[String], arguments: &[CanonicalTypeArgument]) -> ResolvedType {
    let name = path.last().map(|segment| segment.to_ascii_lowercase()).unwrap_or_default();
    let mut unresolved = Vec::new();
    let mut args = Vec::with_capacity(arguments.len());

    for argument in arguments {
        match argument {
            CanonicalTypeArgument::Type(ty) => {
                let resolved = resolve(ty);
                unresolved.extend(resolved.unresolved);
                args.push(resolved.ts);
            }
            CanonicalTypeArgument::Literal(text) => {
                unresolved.push(format!("literal `{text}`"));
                args.push(UNKNOWN.to_string());
            }
        }
    }

    match (name.as_str(), args.len()) {
        ("list" | "array" | "vec", 1) => ResolvedType { ts: format!("{}[]", parenthesize(&args[0])), unresolved },
        ("optional" | "option", 1) => ResolvedType { ts: format!("{} | null", parenthesize(&args[0])), unresolved },
        ("map" | "record" | "dict", 2) => ResolvedType { ts: format!("Record<{}, {}>", args[0], args[1]), unresolved },
        _ => {
            unresolved.push(format!("generic type `{}`", display_path(path)));
            ResolvedType { ts: UNKNOWN.to_string(), unresolved }
        }
    }
}

/// 保守的内建类型映射，只覆盖能安全对应到 JS / TS 原语的名字。
fn builtin(name: &str) -> Option<&'static str> {
    match name {
        "bool" | "boolean" => Some("boolean"),
        "utf8" | "string" | "text" | "str" => Some("string"),
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32" | "f32" | "f64" | "float" | "number" => Some("number"),
        // 64 位与未定宽整数、decimal 与时间类类型在 wire 上按字符串承载，
        // 避免 JS number 精度丢失。codec descriptor 落地后以 descriptor 为准。
        "i64" | "u64" | "int" | "integer" | "bigint" | "decimal" | "uuid" | "date" | "datetime" | "timestamp" => Some("string"),
        "bytes" | "binary" => Some("string"),
        "json" | "value" | "any" => Some(UNKNOWN),
        _ => None,
    }
}

fn display_path(path: &[String]) -> String {
    if path.is_empty() { "<empty path>".to_string() } else { path.join(".") }
}

fn parenthesize(ts: &str) -> String {
    if ts.contains(" | ") { format!("({ts})") } else { ts.to_string() }
}
