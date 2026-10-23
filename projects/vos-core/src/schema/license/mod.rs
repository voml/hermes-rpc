use super::*;

/// https://spdx.github.io/spdx-spec/v2.3/SPDX-license-list/
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum License {
    /// MIT License。
    MIT,
    /// Mozilla Public License 1.0。
    /// <https://spdx.org/licenses/MPL-1.0.html>
    MPL10,
    /// Mozilla Public License 1.1。
    /// <https://spdx.org/licenses/MPL-1.1.html>
    MPL11,
    /// Mozilla Public License 2.0。
    /// <https://spdx.org/licenses/MPL-2.0.html>
    MPL20,
    /// 自定义 license（非 SPDX 标准）。
    Custom {
        /// 自定义 license 名。
        name: String,
        /// 自定义 license 链接。
        link: Url,
    },
}

impl Default for License {
    fn default() -> Self {
        License::MIT
    }
}

impl License {
    /// 返回 license 的显示名。
    pub fn name(&self) -> &str {
        match self {
            License::MIT => "MIT License",
            License::MPL10 => "Mozilla Public License 1.0",
            License::MPL11 => "Mozilla Public License 1.1",
            License::MPL20 => "Mozilla Public License 2.0",
            License::Custom { name, .. } => name.as_str(),
        }
    }
    /// 返回 license 的 SPDX/自定义链接。
    pub fn link(&self) -> &str {
        match self {
            License::MIT => "https://opensource.org/licenses/MIT",
            License::MPL10 => "https://spdx.org/licenses/MPL-1.0.html",
            License::MPL11 => "https://spdx.org/licenses/MPL-1.1.html",
            License::MPL20 => "https://spdx.org/licenses/MPL-2.0.html",
            License::Custom { link, .. } => link.as_str(),
        }
    }
}
