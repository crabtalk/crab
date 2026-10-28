use serde::{Deserialize, Serialize};

/// The one account a local host answers for, in Cloud's paths.
pub const LOCAL: &str = "local";

/// An account on Cloud, by handle or id.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Owner {
    Org(String),
    User(String),
}

impl Owner {
    pub fn name(&self) -> &str {
        match self {
            Owner::Org(name) | Owner::User(name) => name,
        }
    }

    /// Prefix of its fleet routes: its services and their usage, and its
    /// plan's period.
    pub fn path(&self) -> String {
        match self {
            Owner::Org(org) => format!("/orgs/{org}"),
            Owner::User(user) => format!("/users/{user}"),
        }
    }

    /// Prefix of its settings routes: keys, apps and billing. A person reaches
    /// only their own, at `/user`.
    pub fn api(&self) -> String {
        match self {
            Owner::Org(org) => format!("/orgs/{org}"),
            Owner::User(_) => "/user".to_owned(),
        }
    }

    /// The path listing its services.
    pub fn services(&self) -> String {
        format!("{}/services", self.path())
    }
}
