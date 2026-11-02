use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref, str::FromStr};
use url::Url;

/// A service logo's absolute HTTPS URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Logo(Url);

impl FromStr for Logo {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match Url::parse(value) {
            Ok(url) if url.scheme() == "https" && url.has_host() => Ok(Self(url)),
            _ => bail!("logo must be an absolute HTTPS URL"),
        }
    }
}

impl TryFrom<String> for Logo {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<Logo> for String {
    fn from(logo: Logo) -> Self {
        logo.0.into()
    }
}

impl Deref for Logo {
    type Target = Url;

    fn deref(&self) -> &Url {
        &self.0
    }
}

impl fmt::Display for Logo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
