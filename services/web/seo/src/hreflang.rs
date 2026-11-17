use crate::report::{Code, Finding, Level};
use berm_lang::Schema;
use language_tags::LanguageTag;
use scraper::Selector;
use std::collections::{BTreeMap, BTreeSet};
use url::Url;
use webpage::page::Page;

#[derive(Debug, Schema)]
pub struct Alternate {
    pub language: String,
    pub href: Option<String>,
    pub target: Option<String>,
    pub valid_language: bool,
}

impl Alternate {
    pub(crate) fn inspect(page: &Page, findings: &mut Vec<Finding>) -> Vec<Self> {
        let mut alternates = Vec::new();
        for element in page
            .document
            .select(&Selector::parse("head link[rel][hreflang]").expect("static selector"))
        {
            if !element
                .value()
                .attr("rel")
                .unwrap_or_default()
                .split_ascii_whitespace()
                .any(|rel| rel.eq_ignore_ascii_case("alternate"))
            {
                continue;
            }
            let language = element
                .value()
                .attr("hreflang")
                .unwrap_or_default()
                .trim()
                .to_owned();
            let valid_language = language.eq_ignore_ascii_case("x-default")
                || LanguageTag::parse(&language).is_ok_and(|tag| {
                    tag.primary_language().len() == 2
                        && tag.is_valid()
                        && tag.extended_language().is_none()
                        && tag.variant().is_none()
                        && tag.extension().is_none()
                        && tag.private_use().is_none()
                        && tag.region().is_none_or(|region| region.len() == 2)
                });
            let href = element.value().attr("href").map(str::to_owned);
            let target = href
                .as_deref()
                .and_then(|href| Url::parse(href).ok())
                .filter(|url| matches!(url.scheme(), "http" | "https"))
                .map(|url| url.to_string());
            alternates.push(Self {
                language,
                href,
                target,
                valid_language,
            });
        }
        if alternates.iter().any(|item| !item.valid_language) {
            findings.push(Finding::new(Code::InvalidHreflangLanguage, Level::Warning, "An hreflang value is not a supported language, optional script and region, or x-default."));
        }
        if alternates.iter().any(|item| item.target.is_none()) {
            findings.push(Finding::new(
                Code::InvalidHreflangUrl,
                Level::Warning,
                "An hreflang link has no absolute HTTP or HTTPS target.",
            ));
        }
        let mut targets: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
        for item in &alternates {
            if let Some(target) = item.target.as_deref() {
                targets
                    .entry(item.language.to_ascii_lowercase())
                    .or_default()
                    .insert(target);
            }
        }
        if targets.values().any(|targets| targets.len() > 1) {
            findings.push(Finding::new(
                Code::ConflictingHreflang,
                Level::Warning,
                "One hreflang language identifies different target URLs.",
            ));
        }
        alternates
    }
}
