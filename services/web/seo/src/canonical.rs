use crate::report::{Code, Finding, Level};
use berm_lang::Schema;
use scraper::{ElementRef, Selector};
use std::collections::BTreeSet;
use webpage::page::Page;

#[derive(Debug, Schema)]
pub enum Source {
    HtmlHead,
    HtmlBody,
    HttpHeader,
}

#[derive(Debug, Schema)]
pub struct Canonical {
    pub source: Source,
    pub href: Option<String>,
    pub target: Option<String>,
    pub context: String,
}

#[derive(Debug, Schema)]
pub struct LinkHeader {
    pub value: String,
    pub error: Option<String>,
}

impl Canonical {
    pub(crate) fn inspect(
        page: &Page,
        findings: &mut Vec<Finding>,
    ) -> (Vec<Self>, Vec<LinkHeader>) {
        let mut declarations = Vec::new();
        for element in page
            .document
            .select(&Selector::parse("link[rel]").expect("static selector"))
        {
            if !element
                .value()
                .attr("rel")
                .unwrap_or_default()
                .split_ascii_whitespace()
                .any(|rel| rel.eq_ignore_ascii_case("canonical"))
            {
                continue;
            }
            let in_head = element
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(|ancestor| ancestor.value().name() == "head");
            let href = element.value().attr("href").map(str::to_owned);
            let target = href
                .as_deref()
                .and_then(|href| page.base.join(href).ok())
                .filter(|url| matches!(url.scheme(), "http" | "https"))
                .map(|url| url.to_string());
            declarations.push(Self {
                source: if in_head {
                    Source::HtmlHead
                } else {
                    Source::HtmlBody
                },
                href,
                target,
                context: page.url.to_string(),
            });
        }
        let mut headers = Vec::new();
        for (_, value) in page
            .response
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("link"))
        {
            let error = match http_link::parse_link_header(value, &page.url) {
                Ok(links) => {
                    for link in links.into_iter().filter(|link| link.rel == "canonical") {
                        let target = matches!(link.target.scheme(), "http" | "https")
                            .then(|| link.target.to_string());
                        declarations.push(Self {
                            source: Source::HttpHeader,
                            href: Some(link.target.to_string()),
                            target,
                            context: link.context.to_string(),
                        });
                    }
                    None
                }
                Err(error) => Some(error.to_string()),
            };
            headers.push(LinkHeader {
                value: value.clone(),
                error,
            });
        }
        if headers.iter().any(|header| header.error.is_some()) {
            findings.push(Finding::new(
                Code::InvalidLinkHeader,
                Level::Warning,
                "A Link response header could not be parsed; its declarations are not included.",
            ));
        }
        let applicable: Vec<_> = declarations
            .iter()
            .filter(|item| item.applies_to(page))
            .collect();
        if applicable.is_empty() {
            findings.push(Finding::new(
                Code::MissingCanonical,
                Level::Info,
                "No canonical link applicable to this page was found.",
            ));
        }
        if declarations.iter().any(|item| item.target.is_none()) {
            findings.push(Finding::new(
                Code::InvalidCanonical,
                Level::Warning,
                "A canonical link has no valid HTTP or HTTPS target.",
            ));
        }
        if declarations
            .iter()
            .any(|item| matches!(item.source, Source::HtmlBody))
        {
            findings.push(Finding::new(
                Code::CanonicalOutsideHead,
                Level::Warning,
                "A canonical HTML link is outside the head and is not used by this report.",
            ));
        }
        if applicable.len() > 1 {
            findings.push(Finding::new(Code::MultipleCanonicals, Level::Warning, "Multiple canonical links apply to this page; the report uses the first declaration."));
        }
        let targets: BTreeSet<_> = applicable
            .iter()
            .filter_map(|item| item.target.as_deref())
            .collect();
        if targets.len() > 1 {
            findings.push(Finding::new(
                Code::ConflictingCanonicals,
                Level::Warning,
                "Canonical declarations identify different targets.",
            ));
        }
        (declarations, headers)
    }

    pub(crate) fn applies_to(&self, page: &Page) -> bool {
        !matches!(self.source, Source::HtmlBody) && self.context == page.url.as_str()
    }
}
