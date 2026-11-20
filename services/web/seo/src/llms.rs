//! `/llms.txt`, as llmstxt.org proposes it: Markdown opening with an H1 title.

use berm_lang::Schema;
use url::Url;

/// The most links of an `llms.txt` checked for an answer.
pub const CHECKED_LINKS: usize = 10;

#[derive(Debug, Schema)]
pub struct LlmsTxt {
    /// The HTTP status `/llms.txt` answered; absent when nothing answered.
    pub status: Option<u16>,
    /// The H1 the file opens with; absent when the file does not open with one.
    pub title: Option<String>,
    /// Markdown links in the file.
    pub links: usize,
    /// Of the first ten links, those that did not answer 2xx.
    pub broken: Vec<String>,
}

impl LlmsTxt {
    pub fn found(&self) -> bool {
        self.status
            .is_some_and(|status| (200..300).contains(&status))
    }
}

/// The file's opening H1, skipping blank lines.
pub fn title(text: &str) -> Option<String> {
    text.lines()
        .find(|line| !line.trim().is_empty())?
        .strip_prefix("# ")
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
}

/// The targets of the file's Markdown links, resolved against `base`; links
/// that do not resolve to HTTP or HTTPS are left out.
pub fn links(text: &str, base: &Url) -> Vec<Url> {
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("](") {
        rest = &rest[at + 2..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let target = rest[..end].split_whitespace().next().unwrap_or_default();
        if let Ok(url) = base.join(target)
            && matches!(url.scheme(), "http" | "https")
        {
            links.push(url);
        }
        rest = &rest[end..];
    }
    links
}
