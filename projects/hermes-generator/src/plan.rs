//! 生成计划与落盘。
//!
//! 生成器只产出**文件计划**，不直接写盘；调用方用 [`GenerationPlan::write_to`]
//! 落地。文件所有权分两类：`Generated` 每次重建，`Template` 只在缺失时创建。

use std::{fmt, fs, path::Path};

use vos_core::error::HermesError;

/// 生成期错误。
#[derive(Debug)]
pub enum GeneratorError {
    /// 上游 artifact 无法解析或校验失败。
    Artifact(HermesError),
    /// 两个 operation 规范化后得到同一个 TS 标识符。
    NamingCollision {
        /// 冲突的 TS 标识符。
        identifier: String,
        /// 撞名的原始 operation 名。
        operations: Vec<String>,
    },
    /// 写盘失败。
    Io(std::io::Error),
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Artifact(err) => write!(f, "artifact error: {}", err.message()),
            Self::NamingCollision { identifier, operations } => {
                write!(f, "operations {} all normalize to TypeScript identifier `{identifier}`", operations.join(", "))
            }
            Self::Io(err) => write!(f, "io error: {err}"),
        }
    }
}

impl std::error::Error for GeneratorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::NamingCollision { .. } => None,
        }
    }
}

impl From<HermesError> for GeneratorError {
    fn from(err: HermesError) -> Self {
        Self::Artifact(err)
    }
}

impl From<std::io::Error> for GeneratorError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// 文件所有权。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileOwnership {
    /// 派生产物，每次生成都重建。
    Generated,
    /// 一次性业务模板，已存在则保留并在报告中标记跳过。
    Template,
}

/// 生成诊断：不阻断生成，但调用方需要知道生成物在哪降级了。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    message: String,
}

impl Diagnostic {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }

    /// 诊断文本。
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// 一个待写出的文件。路径是相对生成根目录的 POSIX 风格路径。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFile {
    path: String,
    contents: String,
    ownership: FileOwnership,
}

impl GeneratedFile {
    pub(crate) fn new(path: impl Into<String>, contents: impl Into<String>, ownership: FileOwnership) -> Self {
        Self { path: path.into(), contents: contents.into(), ownership }
    }

    /// 相对路径。
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 文件内容。
    pub fn contents(&self) -> &str {
        &self.contents
    }

    /// 所有权。
    pub fn ownership(&self) -> FileOwnership {
        self.ownership
    }
}

/// 一次生成的完整计划。
#[derive(Clone, Debug, Default)]
pub struct GenerationPlan {
    files: Vec<GeneratedFile>,
    diagnostics: Vec<Diagnostic>,
}

impl GenerationPlan {
    pub(crate) fn new(files: Vec<GeneratedFile>, diagnostics: Vec<Diagnostic>) -> Self {
        Self { files, diagnostics }
    }

    /// 待写出的文件。
    pub fn files(&self) -> &[GeneratedFile] {
        &self.files
    }

    /// 生成诊断。
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// 把计划落到 `root` 目录下。
    ///
    /// `Generated` 文件内容未变时不重写（报告记为 unchanged），`Template` 文件
    /// 已存在时一律保留（报告记为 skipped）。
    pub fn write_to(&self, root: &Path) -> Result<WriteReport, GeneratorError> {
        let mut report = WriteReport::default();

        for file in &self.files {
            let path = root.join(&file.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            match file.ownership {
                FileOwnership::Template if path.exists() => {
                    report.skipped.push(file.path.clone());
                    continue;
                }
                FileOwnership::Generated => {
                    if fs::read_to_string(&path).is_ok_and(|existing| existing == file.contents) {
                        report.unchanged.push(file.path.clone());
                        continue;
                    }
                }
                FileOwnership::Template => {}
            }

            fs::write(&path, &file.contents)?;
            report.written.push(file.path.clone());
        }

        Ok(report)
    }
}

/// 落盘结果。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteReport {
    written: Vec<String>,
    unchanged: Vec<String>,
    skipped: Vec<String>,
}

impl WriteReport {
    /// 新建或内容变化的文件。
    pub fn written(&self) -> &[String] {
        &self.written
    }

    /// 内容未变化、未重写的生成文件。
    pub fn unchanged(&self) -> &[String] {
        &self.unchanged
    }

    /// 已存在而被保留的模板文件。
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }
}
