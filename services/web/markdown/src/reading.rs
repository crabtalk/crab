use crate::page::Page;
use berm_lang::{Result, Schema};
use htmd::HtmlToMarkdown;
use scraper::Selector;

const MAX_MARKDOWN: usize = 128 * 1024;

#[derive(Debug, Schema)]
pub struct Reading {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Tag name of the selected content element.
    pub region: String,
    pub markdown: String,
}

impl TryFrom<&Page> for Reading {
    type Error = berm_lang::Error;

    fn try_from(page: &Page) -> Result<Self> {
        let (region, content) = ["main", "[role=main]", "article", "body"]
            .into_iter()
            .find_map(|selector| {
                page.document
                    .select(&Selector::parse(selector).expect("static selector"))
                    .next()
                    .map(|element| (element.value().name().to_owned(), element.html()))
            })
            .ok_or("The page has no readable HTML body")?;
        let converter = HtmlToMarkdown::builder()
            .skip_tags(vec![
                "script", "style", "noscript", "nav", "footer", "aside", "form", "template", "head",
            ])
            .build();
        let markdown = converter
            .convert(&content)
            .map_err(|error| format!("Cannot convert the page: {error}"))?;
        if markdown.trim().is_empty() {
            return Err(
                "The returned HTML contains no readable content; JavaScript is not executed".into(),
            );
        }
        if markdown.len() > MAX_MARKDOWN {
            return Err("The extracted Markdown exceeds 128 KiB; read a smaller page".into());
        }
        Ok(Self {
            url: page.url.to_string(),
            title: page.title(),
            description: page.meta("description").map(str::to_owned),
            region,
            markdown,
        })
    }
}
