use super::*;

/// 项目运行环境（host + 文档）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    /// 环境名（用作 BTreeSet 的 Ord 键）。
    pub name: String,
    /// 目标 host URL。
    pub host: Url,
    /// 环境文档说明。
    pub document: Document,
}

impl Eq for Environment {}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

impl PartialOrd for Environment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Ord for Environment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Environment {
    /// 用 host URL 构造一个名字为 `"default"` 的环境。
    pub fn new(host: Url) -> Self {
        Self { name: "default".to_string(), host, document: Default::default() }
    }
}

impl Project {
    /// 向项目追加一个 environment。
    /// Add a new environment to the project
    pub fn environment(&mut self, environment: Environment) {
        self.environments.insert(environment);
    }
}
