//! ????????
//!
//! ??????**????**??????????? [`GenerationPlan::write_to`]
//! ????????????`Generated` ?????`Template` ????????

use std::{fmt, fs, path::Path};

use vos_types::ArtifactError;

/// ??????
#[derive(Debug)]
pub enum GeneratorError {
    /// ?? artifact ??????????
    Artifact(ArtifactError),
    /// ?? operation ????????? TS ????
    NamingCollision {
        /// ??? TS ????
        identifier: String,
        /// ????? operation ??
        operations: Vec<String>,
    },
    /// ?????
    Io(std::io::Error),
    /// artifact 声明的产品格式不是 Hermes。
    UnsupportedContractFormat {
        /// artifact 实际声明的完整格式字符串。
        found: &'static str,
    },
    /// 两个合同名规范化成同一个 TypeScript 标识符。
    IdentifierCollision {
        /// 规范化后的标识符。
        identifier: String,
        /// 冲突的合同名。
        sources: Vec<String>,
    },
    /// 一个 service 内的两个方法规范化成同一条路由。
    DuplicateRoute {
        /// service 原始名与规范化后的方法标识符组成的路由。
        route: String,
    },
    /// method 绑定的 operation 不在同一 artifact 的 payload 中。
    MissingOperationBinding {
        /// service 原始名。
        service: String,
        /// method 原始名。
        method: String,
        /// 缺失的 operation identity。
        operation_id: String,
    },
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Artifact(err) => write!(f, "artifact error: {}", err.message),
            Self::NamingCollision { identifier, operations } => {
                write!(f, "contract names {} all normalize to TypeScript identifier `{identifier}`", operations.join(", "))
            }
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::UnsupportedContractFormat { found } => {
                write!(f, "generator accepts only `vos/hermes.contract/v0`, found `{found}`")
            }
            Self::IdentifierCollision { identifier, sources } => {
                write!(f, "contract names {} all normalize to TypeScript identifier `{identifier}`", sources.join(", "))
            }
            Self::DuplicateRoute { route } => write!(f, "duplicate route `{route}`"),
            Self::MissingOperationBinding { service, method, operation_id } => {
                write!(f, "method `{service}.{method}` binds operation `{operation_id}` which is not present in the artifact")
            }
        }
    }
}

impl std::error::Error for GeneratorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact(_) => None,
            Self::Io(err) => Some(err),
            Self::NamingCollision { .. }
            | Self::UnsupportedContractFormat { .. }
            | Self::IdentifierCollision { .. }
            | Self::DuplicateRoute { .. }
            | Self::MissingOperationBinding { .. } => None,
        }
    }
}

impl From<ArtifactError> for GeneratorError {
    fn from(err: ArtifactError) -> Self {
        Self::Artifact(err)
    }
}

impl From<std::io::Error> for GeneratorError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// ??????
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileOwnership {
    /// ?????????????
    Generated,
    /// ????????????????????????
    Template,
}

/// ????????????????????????????
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    message: String,
}

impl Diagnostic {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }

    /// ?????
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// ???????????????????? POSIX ?????
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

    /// ?????
    pub fn path(&self) -> &str {
        &self.path
    }

    /// ?????
    pub fn contents(&self) -> &str {
        &self.contents
    }

    /// ????
    pub fn ownership(&self) -> FileOwnership {
        self.ownership
    }
}

/// ??????????
#[derive(Clone, Debug, Default)]
pub struct GenerationPlan {
    files: Vec<GeneratedFile>,
    diagnostics: Vec<Diagnostic>,
}

impl GenerationPlan {
    pub(crate) fn new(files: Vec<GeneratedFile>, diagnostics: Vec<Diagnostic>) -> Self {
        Self { files, diagnostics }
    }

    /// ???????
    pub fn files(&self) -> &[GeneratedFile] {
        &self.files
    }

    /// ?????
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// ????? `root` ????
    ///
    /// `Generated` ??????????????? unchanged??`Template` ??
    /// ????????????? skipped??
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

/// ?????
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteReport {
    written: Vec<String>,
    unchanged: Vec<String>,
    skipped: Vec<String>,
}

impl WriteReport {
    /// ???????????
    pub fn written(&self) -> &[String] {
        &self.written
    }

    /// ???????????????
    pub fn unchanged(&self) -> &[String] {
        &self.unchanged
    }

    /// ?????????????
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }
}
