use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Secrets a person gets together from one provider, such as a GitHub OAuth
/// app's client ID and secret.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    /// What the person sets up, as `GitHub OAuth app`.
    pub title: String,
    /// How to get its secrets, in Markdown.
    #[serde(default)]
    pub guide: String,
    /// Values the person enters at the provider, each label by key. A secret
    /// request gives each value, since they differ per product.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub copy: BTreeMap<String, String>,
    /// The hosts its secrets may be sent to, each named in full.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    /// Whether the service is called with its secrets unset. A header naming
    /// an unset one is left off the request; a body naming one is refused.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// In the order the person sets them.
    pub secrets: Vec<CredentialSecret>,
}

impl Credential {
    pub const HOSTS_MAX: usize = 16;

    pub fn check_hosts(hosts: &[String]) -> Result<(), String> {
        if hosts.len() > Self::HOSTS_MAX {
            return Err(format!(
                "a secret is sent to at most {} hosts",
                Self::HOSTS_MAX
            ));
        }
        if let Some(host) = hosts.iter().find(|host| !Self::host(host)) {
            return Err(format!(
                "{host:?} is not a host named in full, as `api.example.com`: no scheme, port, path or `*`"
            ));
        }
        if let Some((_, host)) = hosts
            .iter()
            .enumerate()
            .find(|(at, host)| hosts[..*at].contains(host))
        {
            return Err(format!("{host} is given twice"));
        }
        Ok(())
    }

    /// Whether `host` names one host in full: lowercase ASCII letters, digits,
    /// `-` and `.`, with no pattern, port or scheme.
    pub fn host(host: &str) -> bool {
        !host.is_empty()
            && host.len() <= 253
            && !host.starts_with(['.', '-'])
            && !host.ends_with(['.', '-'])
            && !host.contains("..")
            && host
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
    }
}

/// One secret of a [`Credential`].
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialSecret {
    pub name: String,
    /// What the provider calls it, as `Client ID`; the name when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Whether the entry field hides the value while it is typed.
    #[serde(default = "masked")]
    pub masked: bool,
    /// A regular expression a value must match; one that does not is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

fn masked() -> bool {
    true
}
