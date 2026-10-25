//! TypeScript 标识符与属性键的规范化。
//!
//! 上游 operation / parameter 名是 VOS 标识符，不能直接假定是合法 TS 标识符。
//! 这里只做机械转换：按非字母数字切成词段，拼成 PascalCase，再据此派生
//! camelCase 方法名。规范化后若两个 operation 撞名，由
//! [`crate::bindings`] 在生成期拒绝，不静默合并。

/// TS / JS 保留字。作为属性名合法，但作为裸标识符（方法简写、参数名）不行。
const RESERVED: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// 是否是可直接裸写的 TS 标识符（会排除保留字）。
pub(crate) fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next()
    else {
        return false;
    };
    !first.is_ascii_digit() && is_word_char(first) && chars.all(is_word_char) && !RESERVED.contains(&value)
}

/// 把任意名字转成 PascalCase 词段拼接。
///
/// `fetch_notes` → `FetchNotes`，`a-b` → `AB`，`A` → `A`。首字符是数字时前缀
/// `_`，无法切出任何词段时返回 `_`。
pub(crate) fn pascal_case(raw: &str) -> String {
    let mut out = String::new();
    for segment in raw.split(|c: char| !c.is_ascii_alphanumeric()) {
        if segment.is_empty() {
            continue;
        }
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// 由 PascalCase 派生 camelCase 方法名。
pub(crate) fn camel_case(raw: &str) -> String {
    let pascal = pascal_case(raw);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => pascal,
    }
}

/// 把名字渲染成 TS 属性键：合法标识符裸写，否则加双引号并转义。
pub(crate) fn property_key(raw: &str) -> String {
    if is_identifier(raw) {
        return raw.to_string();
    }
    string_literal(raw)
}

/// 把任意文本渲染成 TS / JS 双引号字符串字面量。
pub(crate) fn string_literal(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len() + 2);
    escaped.push('"');
    for c in raw.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}
