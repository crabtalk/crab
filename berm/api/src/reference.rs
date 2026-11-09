use anyhow::{Context, Result, bail};
use semver::Version;
use serde::{Deserialize, Deserializer, de::Error};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub owner: String,
    pub name: String,
    pub version: Option<Version>,
}

impl Reference {
    pub fn address(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

impl FromStr for Reference {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let (address, version) = match text.rsplit_once(':') {
            Some((address, version)) => (
                address,
                Some(
                    version
                        .parse()
                        .context("an image version must be exact semantic versioning")?,
                ),
            ),
            None => (text, None),
        };
        let (owner, name) = address
            .split_once('/')
            .context("an image address is owner/service")?;
        if [owner, name].iter().any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        }) {
            bail!(
                "an image address is owner/service, using lowercase letters, digits, `-` and `_`"
            );
        }
        Ok(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
            version,
        })
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)?;
        if let Some(version) = &self.version {
            write!(f, ":{version}")?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Reference {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}
