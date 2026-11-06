use berm_lang::{Result, Schema};
use scraper::Selector;
use webpage::page::Page;

const MAX_HEADINGS: usize = 20;
const MAX_REPORT: usize = 256 * 1024;

#[derive(Debug, PartialEq, Eq, Schema)]
pub enum Level {
    Info,
    Warning,
}

#[derive(Debug, PartialEq, Eq, Schema)]
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
}

#[derive(Debug, Schema)]
pub struct Finding {
    pub code: Code,
    pub level: Level,
    pub message: String,
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
}

impl TryFrom<&Page> for Report {
    type Error = berm_lang::Error;

    fn try_from(page: &Page) -> Result<Self> {
        let title = page.title();
        let description = page.meta("description").map(str::to_owned);
        let titles = page.titles().count();
        let canonicals: Vec<_> = page
            .document
            .select(&Selector::parse("link[rel]").expect("static selector"))
            .filter(|element| {
                element.value().attr("rel").is_some_and(|rel| {
                    rel.split_ascii_whitespace()
                        .any(|part| part.eq_ignore_ascii_case("canonical"))
                })
            })
            .collect();
        let canonical = canonicals
            .first()
            .and_then(|element| element.value().attr("href"))
            .and_then(|href| page.base.join(href).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .map(|url| url.to_string());
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
        let mut robots: Vec<_> = page
            .document
            .select(&Selector::parse("meta[name]").expect("static selector"))
            .filter(|element| {
                element.value().attr("name").is_some_and(|name| {
                    name.eq_ignore_ascii_case("robots") || name.eq_ignore_ascii_case("googlebot")
                })
            })
            .filter_map(|element| element.value().attr("content").map(str::to_owned))
            .collect();
        robots.extend(
            page.response
                .headers
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case("x-robots-tag"))
                .map(|(_, value)| value.clone()),
        );
        let mut findings = Vec::new();
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
        if canonicals.is_empty() {
            findings.push(Finding {
                code: Code::MissingCanonical,
                level: Level::Info,
                message: "No canonical link was declared.".into(),
            });
        } else if canonical.is_none() {
            findings.push(Finding {
                code: Code::InvalidCanonical,
                level: Level::Warning,
                message: "The canonical link has no valid HTTP or HTTPS target.".into(),
            });
        }
        if canonicals.len() > 1 {
            findings.push(Finding {
                code: Code::MultipleCanonicals,
                level: Level::Warning,
                message: "Multiple canonical links were declared; the report uses the first link."
                    .into(),
            });
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
        if robots.iter().any(|value| {
            value
                .split(|ch: char| ch == ',' || ch == ':' || ch.is_ascii_whitespace())
                .any(|part| {
                    part.eq_ignore_ascii_case("noindex") || part.eq_ignore_ascii_case("none")
                })
        }) {
            findings.push(Finding {
                code: Code::Noindex,
                level: Level::Warning,
                message: "A robots directive declares noindex; review whether that is intended."
                    .into(),
            });
        }
        let report = Self {
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
        };
        if berm_lang::serde_json::to_vec(&report)?.len() > MAX_REPORT {
            return Err("The page metadata exceeds the 256 KiB report limit".into());
        }
        Ok(report)
    }
}
