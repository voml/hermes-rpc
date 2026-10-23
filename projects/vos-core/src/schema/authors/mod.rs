use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::str::FromStr;

use email_address::EmailAddress;

use crate::error::{HermesError, QResult};
use crate::schema::objects::Object;
use serde::{Deserialize, Serialize};

use super::Project;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectAuthor {
    pub name: String,
    /// email is the user's primary id
    pub email: EmailAddress,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Object>,
}

impl Eq for ProjectAuthor {}

impl PartialEq<Self> for ProjectAuthor {
    fn eq(&self, other: &Self) -> bool {
        self.email.eq(&other.email)
    }
}

impl PartialOrd<Self> for ProjectAuthor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Ord for ProjectAuthor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl ProjectAuthor {
    pub fn new(name: &str, email: &str) -> QResult<Self> {
        if name.is_empty() {
            return Err(HermesError::new(
                crate::error::ErrorKind::Validation,
                "Project author missing name",
            ));
        }
        if email.is_empty() {
            return Err(HermesError::new(
                crate::error::ErrorKind::Validation,
                "Project author missing email",
            ));
        }
        let email = EmailAddress::from_str(email)?;
        Ok(Self { name: name.to_owned(), email, extra: Default::default() })
    }

    pub fn insert<K, V>(&mut self, key: K, value: V) -> Option<Object>
    where
        K: Into<String>,
        V: Into<Object>,
    {
        self.extra.insert(key.into(), value.into())
    }

    pub fn short_name(&self) -> bool {
        todo!()
    }
}

impl Project {
    #[inline]
    pub fn author(&mut self, author: ProjectAuthor) {
        self.authors.insert(author);
    }
}
