//! span 的 wire 表达与边界校验。
//!
//! 上游 `vos-types` 用 `{start, end}` 对象表达 span，这里保持 1:1 对齐。

use core::range::Range;

use serde::{Deserialize, Serialize};

use crate::error::{ErrorKind, HermesError, QResult};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpanWire {
    start: usize,
    end: usize,
}

pub(super) fn serialize_span<S: serde::Serializer>(span: &Range<usize>, serializer: S) -> Result<S::Ok, S::Error> {
    SpanWire { start: span.start, end: span.end }.serialize(serializer)
}

pub(super) fn deserialize_span<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Range<usize>, D::Error> {
    let span = SpanWire::deserialize(deserializer)?;
    if span.start > span.end {
        return Err(serde::de::Error::custom("span start is after span end"));
    }
    Ok((span.start..span.end).into())
}

/// 校验 span 边界：start <= end、end <= source.len()、start/end 落在 char boundary。
pub(super) fn validate_span(span: &Range<usize>, source: &str, label: &str) -> QResult<()> {
    if span.start > span.end
        || span.end > source.len()
        || !source.is_char_boundary(span.start)
        || !source.is_char_boundary(span.end)
    {
        return Err(HermesError::new(ErrorKind::Validation, format!("{label} out of bounds")));
    }
    Ok(())
}
