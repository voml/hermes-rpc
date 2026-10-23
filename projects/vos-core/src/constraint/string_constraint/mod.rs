use super::*;

/// 字符串类型的约束（同时支持字节长度和 Unicode 字符数两种度量）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StringConstraint {
    /// Minimum length of utf8 string
    pub min_bytes: Option<u32>,
    /// Maximum length of utf8 string
    pub max_bytes: Option<u32>,
    /// Minimum number of unicode characters
    pub min_length: Option<u32>,
    /// Maximum number of unicode characters
    pub max_length: Option<u32>,

    /// 共享的约束元数据。
    #[serde(flatten)]
    pub info: SharedConstraint,
}

impl StringConstraint {
    /// ```vos
    /// type Integer: i32 {
    ///     .min: -1
    /// }
    /// ```
    pub fn min_bytes(&mut self, n: &str) -> QResult {
        self.min_bytes = Some(u32::from_str(n)?);
        Ok(())
    }
    /// ```vos
    /// type Integer: i32 {
    ///     .max: +1
    /// }
    /// ```
    pub fn max_bytes(&mut self, n: &str) -> QResult {
        self.max_bytes = Some(u32::from_str(n)?);
        Ok(())
    }
    /// ```vos
    /// type Positive: i32 {
    ///     .positive
    /// }
    /// ```
    /// ```vos
    /// type Integer: i32 {
    ///     .min: -1
    /// }
    /// ```
    pub fn min_length(&mut self, n: &str) -> QResult {
        self.min_length = Some(u32::from_str(n)?);
        Ok(())
    }
    /// ```vos
    /// type Integer: i32 {
    ///     .max: +1
    /// }
    /// ```
    pub fn max_length(&mut self, n: &str) -> QResult {
        self.max_length = Some(u32::from_str(n)?);
        Ok(())
    }
    /// ```vos
    /// type Positive: i32 {
    ///     .positive
    /// }
    /// ```
    pub fn format(&mut self) {}
}
