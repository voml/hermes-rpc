use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fmt::{Display, Formatter},
    str::FromStr,
};

use semver::Version;
use url::Url;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::*;

pub mod authors;
pub mod document;
pub mod edition;
pub mod endpoint;
pub mod environment;
pub mod license;
pub mod objects;
pub mod route;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub kind: ProjectKind,
    pub license: License,
    pub edition: Version,
    pub version: Version,
    pub authors: BTreeSet<ProjectAuthor>,
    pub description: Document,
    pub environments: BTreeSet<Environment>,
    pub endpoints: BTreeMap<String, Endpoint>,
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

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ProjectKind {
    Server,
    Client,
    Library,
}

impl Default for ProjectKind {
    fn default() -> Self {
        Self::Library
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Module {
    schemas: IndexMap<String, Schema>,
    objects: IndexMap<String, Object>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Schema {
    String(StringConstraint),
    Integer(IntegerConstraint),
    Decimal(DecimalConstraint),
    List(ListConstraint),
    Dict(DictConstraint),
}

impl Project {
    #[inline]
    pub fn extra<K, V>(&mut self, key: K, value: V) -> Option<Object>
    where
        K: Into<String>,
        V: Into<Object>,
    {
        self.extra.insert(key.into(), value.into())
    }
    #[inline]
    pub fn document(&mut self, document: &str) {
        self.description.push(document)
    }
}
