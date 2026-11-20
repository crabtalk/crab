//! Fixed prices for tool calls and successful workflow runs.

use berm_api::ToolSpec;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, str::FromStr};

/// USD micros charged for one priced operation, at least [`Price::MIN`].
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct Price(u64);

impl Price {
    /// The smallest price, $0.0001, in USD micros.
    pub const MIN: u64 = berm_api::ToolSpec::MIN_PRICE;

    pub fn tools(tools: &[ToolSpec]) -> Result<BTreeMap<String, Self>, String> {
        tools
            .iter()
            .filter(|tool| tool.public)
            .filter_map(|tool| tool.price.map(|price| (tool, price)))
            .map(|(tool, price)| Ok((tool.name.clone(), Self::try_from(price)?)))
            .collect()
    }
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

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Tool,
    Workflow,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Tool => "tool",
            Self::Workflow => "workflow",
        })
    }
}

impl FromStr for Kind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "tool" => Ok(Self::Tool),
            "workflow" => Ok(Self::Workflow),
            _ => Err(format!("unknown priced operation kind {value:?}")),
        }
    }
}

impl TryFrom<String> for Kind {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
