use super::*;

/// 十进制数字类型的约束。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecimalConstraint {
    /// 具体的十进制子类型。
    pub kind: DecimalKind,
    /// 下界值。
    pub min: Option<Decimal>,
    /// 下界是否包含（闭区间）。
    pub min_inclusive: bool,
    /// 上界值。
    pub max: Option<Decimal>,
    /// 上界是否包含（闭区间）。
    pub max_inclusive: bool,
    /// 共享的约束元数据。
    #[serde(flatten)]
    pub info: SharedConstraint,
}

/// 十进制子类型枚举。
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum DecimalKind {
    /// 任意精度十进制。
    Decimal,
    /// 8 位精度十进制。
    Decimal8,
    /// 16 位精度十进制。
    Decimal16,
    /// 32 位精度十进制。
    Decimal32,
    /// 64 位精度十进制。
    Decimal64,
    /// 128 位精度十进制。
    Decimal128,
}

impl Default for DecimalKind {
    fn default() -> Self {
        Self::Decimal32
    }
}
