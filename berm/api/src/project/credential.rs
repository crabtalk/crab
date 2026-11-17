use crate::{Credential, CredentialSecret};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub title: Option<String>,
    pub guide: Option<String>,
    pub copy: Option<BTreeMap<String, String>>,
    pub hosts: Option<Vec<String>>,
    pub optional: Option<bool>,
    pub secrets: Option<Vec<CredentialSecret>>,
}
impl Declaration {
    pub fn hosts_only(&self) -> bool {
        self.hosts.is_some()
            && self.title.is_none()
            && self.guide.is_none()
            && self.copy.is_none()
            && self.optional.is_none()
            && self.secrets.is_none()
    }
}
impl From<Declaration> for Credential {
    fn from(value: Declaration) -> Self {
        Self {
            title: value.title.unwrap_or_default(),
            guide: value.guide.unwrap_or_default(),
            copy: value.copy.unwrap_or_default(),
            hosts: value.hosts.unwrap_or_default(),
            optional: value.optional.unwrap_or_default(),
            secrets: value.secrets.unwrap_or_default(),
        }
    }
}
