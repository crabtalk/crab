use berm_lang::Schema;
use scraper::Selector;
use webpage::page::Page;

#[derive(Debug, Schema)]
pub enum Source {
    HtmlMeta,
    HttpHeader,
}

#[derive(Debug, Schema)]
pub struct Directive {
    pub source: Source,
    /// None means the directive is not restricted to a named crawler.
    pub crawler: Option<String>,
    pub value: String,
}

impl Directive {
    pub(crate) fn inspect(page: &Page) -> (Vec<String>, Vec<Self>) {
        let mut raw = Vec::new();
        let mut directives = Vec::new();
        for element in page
            .document
            .select(&Selector::parse("meta[name][content]").expect("static selector"))
        {
            let name = element
                .value()
                .attr("name")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !matches!(
                name.as_str(),
                "robots" | "googlebot" | "googlebot-news" | "bingbot"
            ) {
                continue;
            }
            raw.push(
                element
                    .value()
                    .attr("content")
                    .unwrap_or_default()
                    .to_owned(),
            );
            directives.push(Self {
                source: Source::HtmlMeta,
                crawler: (name != "robots").then_some(name),
                value: element
                    .value()
                    .attr("content")
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
        for (_, value) in page
            .response
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("x-robots-tag"))
        {
            raw.push(value.clone());
            let mut crawler = None;
            for part in value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                let mut rule = part;
                if let Some((prefix, rest)) = part.split_once(':') {
                    let prefix = prefix.trim().to_ascii_lowercase();
                    if !prefix.is_empty()
                        && prefix.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                        })
                        && !matches!(
                            prefix.as_str(),
                            "max-snippet"
                                | "max-image-preview"
                                | "max-video-preview"
                                | "unavailable_after"
                        )
                    {
                        crawler = Some(prefix);
                        rule = rest.trim();
                    }
                }
                directives.push(Self {
                    source: Source::HttpHeader,
                    crawler: crawler.clone(),
                    value: rule.to_owned(),
                });
            }
        }
        (raw, directives)
    }

    pub(crate) fn noindex(&self) -> bool {
        self.value.split(',').any(|part| {
            part.split(':')
                .next()
                .unwrap_or_default()
                .split_ascii_whitespace()
                .any(|token| {
                    token.eq_ignore_ascii_case("noindex") || token.eq_ignore_ascii_case("none")
                })
        })
    }
}
