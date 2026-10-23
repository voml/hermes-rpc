use super::*;

mod der;

/// https://spdx.github.io/spdx-spec/v2.3/SPDX-license-list/
#[derive(Clone, Debug, Serialize)]
pub enum License {
    MIT,
    /// <https://spdx.org/licenses/MPL-1.0.html>
    MPL10,
    /// <https://spdx.org/licenses/MPL-1.1.html>
    MPL11,
    /// <https://spdx.org/licenses/MPL-2.0.html>
    MPL20,
    Custom {
        name: String,
        link: Url,
    },
}

impl Default for License {
    fn default() -> Self {
        License::MIT
    }
}

impl License {
    pub fn name(&self) -> &str {
        match self {
            License::MIT => "MIT License",
            License::MPL10 => "Mozilla Public License 1.0",
            License::MPL11 => "Mozilla Public License 1.1",
            License::MPL20 => "Mozilla Public License 2.0",
            License::Custom { name, .. } => name.as_str(),
        }
    }
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
