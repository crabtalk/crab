use crate::{
    canonical::{Canonical, LinkHeader},
    hreflang::Alternate,
    robots::Directive,
    social::Social,
    structured::Block,
};
use berm_lang::{Result, Schema};
use scraper::Selector;
use std::io::{self, Write};
use webpage::page::Page;

const MAX_HEADINGS: usize = 20;
const MAX_IMAGE_SAMPLES: usize = 20;
const MAX_REPORT: usize = 256 * 1024;

struct ReportLimit(usize);

impl Write for ReportLimit {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_REPORT - self.0 {
            return Err(io::Error::other(
                "The page metadata exceeds the 256 KiB report limit",
            ));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Schema)]
pub enum Level {
    Info,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Schema)]
pub enum Code {
    MissingTitle,
    MultipleTitles,
    MissingDescription,
    MissingCanonical,
    MultipleCanonicals,
    InvalidCanonical,
    MissingLanguage,
    MissingH1,
    MultipleH1,
    MissingImageAlt,
    Noindex,
    MultipleDescriptions,
    InvalidLinkHeader,
    CanonicalOutsideHead,
    ConflictingCanonicals,
    InvalidStructuredData,
    MissingSocialMetadata,
    InvalidSocialUrl,
    InvalidTwitterCard,
    ConflictingSocialMetadata,
    InvalidHreflangLanguage,
    InvalidHreflangUrl,
    ConflictingHreflang,
}

#[derive(Debug, Schema)]
pub struct Finding {
    pub code: Code,
    pub level: Level,
    pub message: String,
}

impl Finding {
    pub(crate) fn new(code: Code, level: Level, message: impl Into<String>) -> Self {
        Self {
            code,
            level,
            message: message.into(),
        }
    }
}

#[derive(Debug, Schema)]
pub struct Image {
    /// One-based position among all img elements.
    pub index: usize,
    pub src: Option<String>,
    pub srcset: Option<String>,
}

#[derive(Debug, Schema)]
pub struct Report {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical: Option<String>,
    pub language: Option<String>,
    pub h1: Vec<String>,
    pub h1_count: usize,
    pub images: usize,
    pub images_without_alt: usize,
    pub robots: Vec<String>,
    pub findings: Vec<Finding>,
    pub descriptions: Vec<Option<String>>,
    pub canonicals: Vec<Canonical>,
    pub link_headers: Vec<LinkHeader>,
    pub structured_data: Vec<Block>,
    pub social: Social,
    pub robots_directives: Vec<Directive>,
    /// At most twenty examples, alongside images_without_alt's full count.
    pub image_alt_samples: Vec<Image>,
    pub hreflang: Vec<Alternate>,
}

impl TryFrom<&Page> for Report {
    type Error = berm_lang::Error;

    fn try_from(page: &Page) -> Result<Self> {
        let report = Self::inspect(page);
        berm_lang::serde_json::to_writer(ReportLimit(0), &report)?;
        Ok(report)
    }
}

impl Report {
    pub(crate) fn inspect(page: &Page) -> Self {
        let title = page.title();
        let description = page.meta("description").map(str::to_owned);
        let titles = page.titles().count();
        let descriptions = page
            .metas("description")
            .map(|element| element.value().attr("content").map(str::to_owned))
            .collect::<Vec<_>>();
        let mut findings = Vec::new();
        let (canonicals, link_headers) = Canonical::inspect(page, &mut findings);
        let canonical = canonicals
            .iter()
            .find(|item| item.applies_to(page))
            .and_then(|item| item.target.clone());
        let structured_data = Block::inspect(page, &mut findings);
        let social = Social::inspect(page, &mut findings);
        let (robots, robots_directives) = Directive::inspect(page);
        let hreflang = Alternate::inspect(page, &mut findings);
        let image_alt_samples = page
            .document
            .select(&Selector::parse("img").expect("static selector"))
            .enumerate()
            .filter(|(_, element)| element.value().attr("alt").is_none())
            .take(MAX_IMAGE_SAMPLES)
            .map(|(index, element)| Image {
                index: index + 1,
                src: element.value().attr("src").map(str::to_owned),
                srcset: element.value().attr("srcset").map(str::to_owned),
            })
            .collect();
        let language = page
            .document
            .root_element()
            .value()
            .attr("lang")
            .map(str::trim)
            .filter(|lang| !lang.is_empty())
            .map(str::to_owned);
        let headings: Vec<_> = page
            .document
            .select(&Selector::parse("h1").expect("static selector"))
            .map(|element| element.text().collect::<String>().trim().to_owned())
            .collect();
        let images = page
            .document
            .select(&Selector::parse("img").expect("static selector"))
            .count();
        let images_without_alt = page
            .document
            .select(&Selector::parse("img:not([alt])").expect("static selector"))
            .count();
        if title.is_none() {
            findings.push(Finding {
                code: Code::MissingTitle,
                level: Level::Warning,
                message: "The returned HTML has no nonempty page title.".into(),
            });
        }
        if titles > 1 {
            findings.push(Finding {
                code: Code::MultipleTitles,
                level: Level::Warning,
                message: format!("The returned HTML contains {titles} title elements."),
            });
        }
        if description.is_none() {
            findings.push(Finding {
                code: Code::MissingDescription,
                level: Level::Info,
                message: "No meta description was found in the returned HTML.".into(),
            });
        }
        if descriptions.len() > 1 {
            findings.push(Finding::new(
                Code::MultipleDescriptions,
                Level::Warning,
                format!(
                    "The returned HTML contains {} meta descriptions.",
                    descriptions.len()
                ),
            ));
        }
        if language.is_none() {
            findings.push(Finding {
                code: Code::MissingLanguage,
                level: Level::Info,
                message: "The HTML element has no language declaration.".into(),
            });
        }
        if headings.is_empty() || headings.iter().all(String::is_empty) {
            findings.push(Finding {
                code: Code::MissingH1,
                level: Level::Info,
                message: "No nonempty h1 heading was found.".into(),
            });
        } else if headings.len() > 1 {
            findings.push(Finding {
                code: Code::MultipleH1,
                level: Level::Info,
                message: format!(
                    "The page has {} h1 headings; review its heading structure.",
                    headings.len()
                ),
            });
        }
        if images_without_alt > 0 {
            findings.push(Finding { code: Code::MissingImageAlt, level: Level::Info, message: format!("{images_without_alt} images have no alt attribute; empty alt attributes are retained for decorative images.") });
        }
        for directive in robots_directives
            .iter()
            .filter(|directive| directive.noindex())
        {
            findings.push(Finding::new(
                Code::Noindex,
                Level::Warning,
                format!(
                    "A robots directive declares noindex for {}; review whether that is intended.",
                    directive.crawler.as_deref().unwrap_or("all crawlers")
                ),
            ));
        }
        Self {
            url: page.url.to_string(),
            title,
            description,
            canonical,
            language,
            h1_count: headings.len(),
            h1: headings.into_iter().take(MAX_HEADINGS).collect(),
            images,
            images_without_alt,
            robots,
            findings,
            descriptions,
            canonicals,
            link_headers,
            structured_data,
            social,
            robots_directives,
            image_alt_samples,
            hreflang,
        }
    }
}
