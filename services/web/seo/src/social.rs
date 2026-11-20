use crate::report::{Code, Finding, Level};
use berm_lang::Schema;
use scraper::Selector;
use std::collections::{BTreeMap, BTreeSet};
use url::Url;
use webpage::page::Page;

const OG_REQUIRED: [&str; 4] = ["og:title", "og:type", "og:image", "og:url"];
const CARDS: [&str; 4] = ["summary", "summary_large_image", "player", "app"];

#[derive(Debug, Schema)]
pub struct Property {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Schema)]
pub struct Social {
    pub properties: Vec<Property>,
    pub missing: Vec<String>,
    pub invalid_urls: Vec<String>,
    pub conflicting: Vec<String>,
}

impl Social {
    pub(crate) fn inspect(page: &Page, findings: &mut Vec<Finding>) -> Self {
        let mut properties = Vec::new();
        for element in page
            .document
            .select(&Selector::parse("head meta").expect("static selector"))
        {
            for key in ["property", "name"] {
                let Some(name) = element.value().attr(key).map(str::to_ascii_lowercase) else {
                    continue;
                };
                if !name.starts_with("og:") && !name.starts_with("twitter:") {
                    continue;
                }
                properties.push(Property {
                    name,
                    value: element
                        .value()
                        .attr("content")
                        .unwrap_or_default()
                        .trim()
                        .to_owned(),
                });
            }
        }
        let values = |name: &'static str| {
            properties
                .iter()
                .filter(move |item| item.name == name && !item.value.is_empty())
        };
        let mut missing = Vec::new();
        for name in OG_REQUIRED {
            if values(name).next().is_none() {
                missing.push(name.to_owned());
            }
        }
        let card = values("twitter:card")
            .next()
            .map(|item| item.value.as_str());
        if card.is_none() {
            missing.push("twitter:card".into());
        }
        if card.is_some() {
            for (twitter, og) in [
                ("twitter:title", "og:title"),
                ("twitter:description", "og:description"),
            ] {
                if values(twitter).next().is_none() && values(og).next().is_none() {
                    missing.push(twitter.into());
                }
            }
            if card == Some("summary_large_image")
                && values("twitter:image").next().is_none()
                && values("og:image").next().is_none()
            {
                missing.push("twitter:image".into());
            }
            if card == Some("player") {
                for name in [
                    "twitter:player",
                    "twitter:player:width",
                    "twitter:player:height",
                ] {
                    if values(name).next().is_none() {
                        missing.push(name.into());
                    }
                }
            }
        }
        let mut invalid_urls = BTreeSet::new();
        let mut scalar_values: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for item in &properties {
            if matches!(
                item.name.as_str(),
                "og:url"
                    | "og:image"
                    | "og:image:url"
                    | "og:image:secure_url"
                    | "twitter:image"
                    | "twitter:player"
            ) && !Url::parse(&item.value)
                .is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
            {
                invalid_urls.insert(item.name.clone());
            }
            if matches!(
                item.name.as_str(),
                "og:title"
                    | "og:type"
                    | "og:url"
                    | "og:description"
                    | "twitter:card"
                    | "twitter:title"
                    | "twitter:description"
                    | "twitter:image"
            ) {
                scalar_values
                    .entry(&item.name)
                    .or_default()
                    .insert(&item.value);
            }
        }
        let conflicting = scalar_values
            .into_iter()
            .filter(|(_, values)| values.len() > 1)
            .map(|(name, _)| name.to_owned())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            findings.push(Finding::new(
                Code::MissingSocialMetadata,
                Level::Info,
                format!(
                    "Social preview metadata is incomplete: {}.",
                    missing.join(", ")
                ),
            ));
        }
        if !invalid_urls.is_empty() {
            findings.push(Finding::new(Code::InvalidSocialUrl, Level::Warning, "Social metadata contains an image, page or player URL that is not an absolute HTTP or HTTPS URL."));
        }
        if card.is_some_and(|card| !CARDS.contains(&card)) {
            findings.push(Finding::new(
                Code::InvalidTwitterCard,
                Level::Warning,
                "The twitter:card value is not a supported card type.",
            ));
        }
        if !conflicting.is_empty() {
            findings.push(Finding::new(Code::ConflictingSocialMetadata, Level::Info, "Repeated scalar social properties contain different values; review preview selection."));
        }
        Self {
            properties,
            missing,
            invalid_urls: invalid_urls.into_iter().collect(),
            conflicting,
        }
    }
}
