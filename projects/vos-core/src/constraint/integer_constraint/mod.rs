use super::*;

/// 整数类型的约束（支持有符号/无符号的多种位宽）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IntegerConstraint {
    /// 整数子类型（位宽 + 有符号性）。
    pub kind: IntegerKind,
    /// 最小值（闭区间）。
    pub min: Option<BigInt>,
    /// 最大值（闭区间）。
    pub max: Option<BigInt>,
    /// 最小值别名（语义更清晰的写法）。
    pub min_length: Option<BigInt>,
    /// 最大值别名（语义更清晰的写法）。
    pub max_length: Option<BigInt>,
    /// 必须是某个值的倍数。
    pub multiple_of: Option<BigInt>,
    /// 示例值列表。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<BigInt>,
    /// 共享的约束元数据。
    #[serde(flatten)]
    pub info: SharedConstraint,
}

/// 整数子类型枚举，覆盖常见位宽和有符号性。
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum IntegerKind {
    /// 任意精度整数。
    Integer,
    /// 8 位有符号整数。
    Integer8,
    /// 16 位有符号整数。
    Integer16,
    /// 32 位有符号整数。
    Integer32,
    /// 64 位有符号整数。
    Integer64,
    /// 128 位有符号整数。
    Integer128,
    /// 256 位有符号整数。
    Integer256,
    /// 8 位无符号整数。
    Unsigned8,
    /// 16 位无符号整数。
    Unsigned16,
    /// 32 位无符号整数。
    Unsigned32,
    /// 64 位无符号整数。
    Unsigned64,
    /// 128 位无符号整数。
    Unsigned128,
    /// 256 位无符号整数。
    Unsigned256,
}

impl Default for IntegerKind {
    fn default() -> Self {
        Self::Integer32
    }
}

impl IntegerConstraint {}

impl IntegerConstraint {
    /// ```vos
    /// i: i32[>1];
    /// i: i32[>=2];
    /// type Integer: i32 {
    ///     .min: -1
    /// }
    /// ```
    pub fn min(&mut self, n: &str, inclusive: bool) -> QResult {
        let mut limit = BigInt::from_str(n)?;
        if !inclusive {
            limit += 1;
        }
        self.min = Some(limit);
        Ok(())
    }
    /// ```vos
    /// i: i32[<1];
    /// i: i32[<=0];
    /// type Integer: i32 {
    ///     .max: +1
    /// }
    /// ```
    pub fn max(mut self, n: &str, inclusive: bool) -> QResult {
        let mut limit = BigInt::from_str(n)?;
        if !inclusive {
            limit -= 1;
        }
        self.max = Some(limit);
        Ok(())
    }
    /// ```vos
    /// type Positive: i32 {
    ///     .positive
    /// }
    /// ```
    pub fn positive(mut self) -> QResult {
        self.min = Some(BigInt::zero());
        Ok(())
    }
    /// ```vos
    /// type Positive: i32 {
    ///     .positive
    /// }
    /// ```
    pub fn negative(mut self) -> QResult {
        self.max = Some(BigInt::zero());
        Ok(())
    }
}
