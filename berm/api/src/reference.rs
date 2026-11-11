use crate::address::Address;
use anyhow::{Context, Result};
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
        let Address { owner, name } = address.parse()?;
        Ok(Self {
            owner,
            name,
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
