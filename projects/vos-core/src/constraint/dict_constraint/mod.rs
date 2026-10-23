use super::*;

/// 字典（key → value）类型的约束。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DictConstraint {
    /// 字典最小条目数。
    pub min_length: Option<u32>,
    /// 字典最大条目数。
    pub max_length: Option<u32>,
    /// 共享的约束元数据。
    #[serde(flatten)]
    pub info: SharedConstraint,
}
