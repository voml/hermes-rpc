use super::*;
use crate::List;

/// 列表类型的约束。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ListConstraint {
    /// 列表最小元素数。
    pub min_length: Option<u32>,
    /// 列表最大元素数。
    pub max_length: Option<u32>,
    /// 示例元素列表。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<List>,
    /// 共享的约束元数据。
    #[serde(flatten)]
    pub info: SharedConstraint,
}
