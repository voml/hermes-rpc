//! `ResolvedCanonicalType` → TypeScript 类型表达式。
//!
//! 合同 artifact 只携带 durable identity 与 canonical type，不携带用户类型的字段
//! 形状。因此 user 类型映射成携带 `type_id` 的 nominal 引用，并产生明确的诊断。
//! 其余分支必须逐项可映射：标量与 `Reference` / `Optional` / `List` 包装直接转写，
//! 已知泛型按容器转写。未知泛型、literal 参数与未冻结的 `json` / `value` / `any`
//! 返回结构化 [`GeneratorError`]，绝不降级成 `unknown` 后宣称生成成功。

use std::collections::BTreeMap;

use vos_types::{ArtifactError, CanonicalScalar, ResolvedCanonicalType, ResolvedCanonicalTypeArgument};

use crate::{naming::pascal_case, plan::GeneratorError};

/// 未冻结类型名：不能安全映射到 TypeScript，必须显式拒绝。
const UNFROZEN_TYPES: [&str; 3] = ["json", "value", "any"];

/// 错误码：artifact 声明了未冻结的 `json` / `value` / `any` 类型。
const UNFROZEN_TYPE_CODE: &str = "GEN001";
/// 错误码：未知 generic 或参数个数不匹配。
const UNSUPPORTED_GENERIC_CODE: &str = "GEN002";
/// 错误码：无法转写的 literal generic 参数。
const LITERAL_ARGUMENT_CODE: &str = "GEN003";

/// 一次类型映射的结果。
pub(crate) struct MappedType {
    /// TS 类型表达式。
    pub(crate) ts: String,
    /// 无法完整映射的说明，调用方补上 service / method 上下文。
    pub(crate) issues: Vec<String>,
}

/// 生成物里引用到的 user 类型。
pub(crate) struct UserTypeBinding {
    /// 生成的 TS 类型别名。
    pub(crate) name: String,
    /// 上游 canonical path。
    pub(crate) path: String,
    /// durable type id。
    pub(crate) type_id: u64,
}

/// user 类型集合，按 TS 别名稳定排序。
#[derive(Default)]
pub(crate) struct UserTypes {
    by_path: BTreeMap<String, UserTypeBinding>,
    names: BTreeMap<String, Vec<String>>,
}

impl UserTypes {
    /// 记录一个 user 类型并返回它的 TS 别名。
    pub(crate) fn bind(&mut self, path: &[String], type_id: u64) -> String {
        let path_text = display_path(path);
        let name = pascal_case(&path_text);
        let paths = self.names.entry(name.clone()).or_default();
        if !paths.contains(&path_text) {
            paths.push(path_text.clone());
        }
        self.by_path.entry(path_text.clone()).or_insert(UserTypeBinding { name: name.clone(), path: path_text, type_id });
        name
    }

    /// 按别名排序后的全部 user 类型。
    pub(crate) fn into_bindings(self) -> Vec<UserTypeBinding> {
        let mut bindings = self.by_path.into_values().collect::<Vec<_>>();
        bindings.sort_by(|left, right| left.name.cmp(&right.name));
        bindings
    }

    /// 别名冲突：同一个 TS 别名对应多个 canonical path。
    pub(crate) fn collision(&self) -> Option<(String, Vec<String>)> {
        self.names.iter().find(|(_, paths)| paths.len() > 1).map(|(name, paths)| (name.clone(), paths.clone()))
    }
}

/// 映射一个 canonical type。
pub(crate) fn map(ty: &ResolvedCanonicalType, users: &mut UserTypes) -> Result<MappedType, GeneratorError> {
    match ty {
        ResolvedCanonicalType::Scalar(scalar) => Ok(MappedType { ts: scalar_ts(*scalar).to_string(), issues: Vec::new() }),
        ResolvedCanonicalType::User { path, type_id } => {
            reject_unfrozen(path)?;
            let name = users.bind(path, *type_id);
            Ok(MappedType {
                ts: name.clone(),
                issues: vec![format!(
                    "user type `{}` carries no field shape in the contract artifact, emitted nominal reference `{name}`",
                    display_path(path)
                )],
            })
        }
        ResolvedCanonicalType::Reference(inner) => map(inner, users),
        ResolvedCanonicalType::Optional(inner) => {
            let inner = map(inner, users)?;
            Ok(MappedType { ts: format!("{} | null", parenthesize(&inner.ts)), issues: inner.issues })
        }
        ResolvedCanonicalType::List(inner) => {
            let inner = map(inner, users)?;
            Ok(MappedType { ts: format!("{}[]", parenthesize(&inner.ts)), issues: inner.issues })
        }
        ResolvedCanonicalType::Generic { path, arguments } => map_generic(path, arguments, users),
    }
}

fn map_generic(
    path: &[String],
    arguments: &[ResolvedCanonicalTypeArgument],
    users: &mut UserTypes,
) -> Result<MappedType, GeneratorError> {
    reject_unfrozen(path)?;

    let name = path.last().map(|segment| segment.to_ascii_lowercase()).unwrap_or_default();
    let mut issues = Vec::new();
    let mut args = Vec::with_capacity(arguments.len());

    for argument in arguments {
        match argument {
            ResolvedCanonicalTypeArgument::Type(ty) => {
                let mapped = map(ty, users)?;
                issues.extend(mapped.issues);
                args.push(mapped.ts);
            }
            ResolvedCanonicalTypeArgument::Literal(text) => {
                return Err(mapping_error(
                    LITERAL_ARGUMENT_CODE,
                    format!("literal generic argument `{text}` cannot be mapped to TypeScript"),
                ));
            }
        }
    }

    let ts = match (name.as_str(), args.as_slice()) {
        ("list" | "array" | "vec", [single]) => format!("{}[]", parenthesize(single)),
        ("optional" | "option", [single]) => format!("{} | null", parenthesize(single)),
        ("map" | "record" | "dict", [key, value]) => format!("Record<{key}, {value}>"),
        _ => {
            return Err(mapping_error(
                UNSUPPORTED_GENERIC_CODE,
                format!("generic type `{}` cannot be mapped to TypeScript", display_path(path)),
            ));
        }
    };

    Ok(MappedType { ts, issues })
}

/// 拒绝上游尚未冻结、也没有稳定 TS 语义的类型名。
fn reject_unfrozen(path: &[String]) -> Result<(), GeneratorError> {
    let Some(name) = path.last()
    else {
        return Ok(());
    };
    let name = name.to_ascii_lowercase();
    if UNFROZEN_TYPES.contains(&name.as_str()) {
        return Err(mapping_error(
            UNFROZEN_TYPE_CODE,
            format!("unfrozen type `{}` cannot be mapped to TypeScript", display_path(path)),
        ));
    }
    Ok(())
}

fn mapping_error(code: &str, message: String) -> GeneratorError {
    GeneratorError::Artifact(ArtifactError { code: code.to_owned(), message })
}

fn scalar_ts(scalar: CanonicalScalar) -> &'static str {
    match scalar {
        CanonicalScalar::I8
        | CanonicalScalar::I16
        | CanonicalScalar::I32
        | CanonicalScalar::U8
        | CanonicalScalar::U16
        | CanonicalScalar::U32
        | CanonicalScalar::F32
        | CanonicalScalar::F64 => "number",
        CanonicalScalar::Bool => "boolean",
        CanonicalScalar::Utf8 | CanonicalScalar::Utf16 => "string",
        // 64 位整数、decimal 与时间类 scalar 在 wire 上按字符串承载，避免 JS
        // number 精度丢失。codec descriptor 落地后以 descriptor 为准。
        CanonicalScalar::I64
        | CanonicalScalar::U64
        | CanonicalScalar::Decimal
        | CanonicalScalar::Uuid
        | CanonicalScalar::Date
        | CanonicalScalar::Time
        | CanonicalScalar::DateTime => "string",
        // bytes 在 canonical wire 上用 u8 数组承载，TS 面直接映射为 Uint8Array
        CanonicalScalar::Bytes => "Uint8Array",
    }
}

fn display_path(path: &[String]) -> String {
    if path.is_empty() { "<empty path>".to_string() } else { path.join(".") }
}

fn parenthesize(ts: &str) -> String {
    if ts.contains(" | ") { format!("({ts})") } else { ts.to_string() }
}
