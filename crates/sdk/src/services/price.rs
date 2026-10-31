//! A tool's price a call, as a deploy sets it from `[tools]` in `crab.toml`.

use berm_api::Manifest;
use serde::{Deserialize, Serialize};
use std::fmt;

/// USD micros a call of a tool costs its caller, at least [`Price::MIN`].
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct Price(u64);

impl Price {
    /// The smallest price, $0.0001, in USD micros.
    pub const MIN: u64 = 100;
}

impl TryFrom<u64> for Price {
    type Error = String;

    fn try_from(micros: u64) -> Result<Self, String> {
        match micros {
            micros if micros < Self::MIN => Err(format!(
                "a price is at least $0.0001, {} USD micros, not {micros}",
                Self::MIN
            )),
            micros => Ok(Self(micros)),
        }
    }
}

impl From<Price> for u64 {
    fn from(price: Price) -> u64 {
        price.0
    }
}

/// USD to the micro, with no zeros past the cent: `$0.002`, `$1.50`.
impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fraction = format!("{:06}", self.0 % 1_000_000);
        let trimmed = fraction.trim_end_matches('0');
        let cents = &fraction[..trimmed.len().max(2)];
        write!(f, "${}.{cents}", self.0 / 1_000_000)
    }
}

/// Refused naming the first of `priced` that is not a public tool of
/// `manifest`.
pub fn check<'a>(
    manifest: &Manifest,
    priced: impl IntoIterator<Item = &'a String>,
) -> Result<(), String> {
    for tool in priced {
        if !manifest
            .tools
            .iter()
            .any(|spec| spec.public && spec.name == *tool)
        {
            return Err(format!(
                "a price names {tool:?}, which is not a public tool of this image"
            ));
        }
    }
    Ok(())
}
