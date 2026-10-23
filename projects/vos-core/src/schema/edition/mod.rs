use super::*;

impl Project {
    /// 设置项目的 edition（major + minor）。
    pub fn edition(&mut self, major: u64, minor: u64) {
        self.edition = Version { major, minor, patch: 0, pre: Default::default(), build: Default::default() }
    }
    /// 从字符串解析项目版本并设置。
    pub fn version(&mut self, v: &str) -> QResult {
        self.version = Version::from_str(v)?;
        Ok(())
    }
}
