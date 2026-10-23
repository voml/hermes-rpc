use super::*;

/// 项目文档块（GFM markdown）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Document {
    /// 文档格式（当前只有 GFM）。
    pub kind: DocumentKind,
    /// 文档纯文本内容。
    pub text: String,
}

/// 支持的文档格式枚举。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DocumentKind {
    /// GitHub Flavored Markdown。
    GFM,
}

impl Default for DocumentKind {
    fn default() -> Self {
        Self::GFM
    }
}

impl Display for Document {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for line in self.text.lines() {
            writeln!(f, "/// {}", line)?
        }
        Ok(())
    }
}

impl Document {
    /// 构造一个 `DocumentKind::GFM` 的文档。
    pub fn markdown(text: impl Into<String>) -> Self {
        Self { kind: DocumentKind::GFM, text: text.into() }
    }
    /// 追加一段文档文本（空字符串会被忽略，非空会在末尾加换行）。
    pub fn push(&mut self, document: &str) {
        if document.trim().is_empty() {
            return;
        }
        if !self.text.is_empty() {
            self.text.push('\n')
        }
        self.text.push_str(document);
    }
}
