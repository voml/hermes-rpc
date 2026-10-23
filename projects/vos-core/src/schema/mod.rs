use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt::{Display, Formatter},
    str::FromStr,
};

use semver::Version;
use url::Url;

use serde::{Deserialize, Serialize};

use crate::*;

pub mod authors;
pub mod document;
pub mod edition;
pub mod endpoint;
pub mod environment;
pub mod license;
pub mod objects;

/// VOS 项目声明的顶层容器。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    /// 项目 kind（server / client / library）。
    pub kind: ProjectKind,
    /// 项目使用的 license。
    pub license: License,
    /// 语言 edition（语义化版本）。
    pub edition: Version,
    /// 项目自身的语义化版本。
    pub version: Version,
    /// 项目作者集合。
    pub authors: BTreeSet<ProjectAuthor>,
    /// 项目文档块（GFM markdown）。
    pub description: Document,
    /// 项目声明的所有 environment。
    pub environments: BTreeSet<Environment>,
    /// 项目对外暴露的 endpoint（按名字索引）。
    pub endpoints: BTreeMap<String, Endpoint>,
    /// 额外的扩展字段（schema 未预留的任意类型）。
    pub extra: BTreeMap<String, Object>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            kind: Default::default(),
            license: Default::default(),
            edition: Version::new(2020, 0, 0),
            version: Version::new(0, 0, 0),
            authors: Default::default(),
            description: Default::default(),
            environments: Default::default(),
            endpoints: Default::default(),
            extra: Default::default(),
        }
    }
}

/// 项目 kind 枚举。
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ProjectKind {
    /// 服务端项目（通常暴露 RPC 接口）。
    Server,
    /// 客户端项目（通常消费 RPC 接口）。
    Client,
    /// 库项目（仅提供类型与约束，不做部署）。
    Library,
}

impl Default for ProjectKind {
    fn default() -> Self {
        Self::Library
    }
}

/// 约束 schema 枚举，tag 为 `type`，携带具体的约束类型。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Schema {
    /// 字符串约束。
    String(StringConstraint),
    /// 整数约束。
    Integer(IntegerConstraint),
    /// 十进制约束。
    Decimal(DecimalConstraint),
    /// 列表约束。
    List(ListConstraint),
    /// 字典约束。
    Dict(DictConstraint),
}

impl Project {
    /// 向额外字段 map 中插入一个值，返回旧值（若有）。
    #[inline]
    pub fn extra<K, V>(&mut self, key: K, value: V) -> Option<Object>
    where
        K: Into<String>,
        V: Into<Object>,
    {
        self.extra.insert(key.into(), value.into())
    }
    /// 向项目文档块追加一段文本。
    #[inline]
    pub fn document(&mut self, document: &str) {
        self.description.push(document)
    }
}
