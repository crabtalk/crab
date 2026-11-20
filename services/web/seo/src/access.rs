//! Whether AI crawlers can reach and read a site: its `robots.txt` rules for
//! them, its `llms.txt`, whether its root page's content is in the HTML
//! response, and whether pages have Markdown copies.

use crate::{
    crawlers::{AGENTS, Crawler, Robots, Rule},
    llms::{self, CHECKED_LINKS, LlmsTxt},
    report::{Level, Report},
};
use berm_lang::{Result, Schema, http};
use scraper::{Node, Selector};
use url::Url;
use webpage::page::Page;

const TIMEOUT: u64 = 10_000;
const MAX_TEXT_FILE: usize = 512 * 1024;
/// Characters of visible text below which a page that runs scripts is taken
/// to fill its content in with JavaScript.
const SHELL_TEXT: usize = 200;

#[derive(Debug, PartialEq, Eq, Schema)]
pub enum Code {
    BlocksAiCrawler,
    MissingLlmsTxt,
    InvalidLlmsTxt,
    BrokenLlmsTxtLink,
    ContentNeedsJavaScript,
    NoMarkdownCopy,
}

#[derive(Debug, Schema)]
pub struct Finding {
    pub code: Code,
    pub level: Level,
    pub message: String,
}

#[derive(Debug, Schema)]
pub struct Access {
    /// The root page's final URL.
    pub url: String,
    /// The HTTP status `/robots.txt` answered; absent when nothing answered.
    pub robots_status: Option<u16>,
    /// Each AI crawler's rule for the root page's path.
    pub crawlers: Vec<Crawler>,
    pub llms: LlmsTxt,
    /// Characters of visible text in the root page's HTML response.
    pub text: usize,
    /// Script elements in it, JSON-LD left out.
    pub scripts: usize,
    /// Whether the root page answers at its URL with `.md` appended.
    pub markdown_suffix: bool,
    /// Whether the root page answers `Accept: text/markdown` with Markdown.
    pub markdown_accept: bool,
    pub findings: Vec<Finding>,
    /// The root page's metadata checks, as `check` reports them.
    pub page: Report,
}

impl Access {
    pub fn inspect(site: &str) -> Result<Self> {
        let page = Page::fetch(site)?;
        let report = Report::try_from(&page)?;
        let url = page.url.clone();
        let at = |path| url.join(path).map_err(|_| "The page URL has no origin");
        let (robots_url, llms_url) = (at("/robots.txt")?, at("/llms.txt")?);
        let suffix_url = markdown_copy(&url);
        let mut answers = http::fetch_all(&[
            get(&robots_url, &[], MAX_TEXT_FILE),
            get(&llms_url, &[], MAX_TEXT_FILE),
            get(&suffix_url, &[], 1),
            get(&url, &[("Accept", "text/markdown")], 1),
        ])?
        .into_iter()
        .map(Result::ok);
        let mut answer = || answers.next().flatten();
        let (robots, llms_file, suffix, accept) = (answer(), answer(), answer(), answer());

        let rules = robots
            .as_ref()
            .filter(|response| response.is_success())
            .map(|response| Robots::from(String::from_utf8_lossy(&response.body).as_ref()));
        let crawlers: Vec<_> = AGENTS
            .iter()
            .map(|agent| match &rules {
                Some(rules) => rules.crawler(agent, url.path()),
                None => Crawler {
                    agent: (*agent).into(),
                    rule: Rule::Unspecified,
                    group: None,
                },
            })
            .collect();

        let mut llms = LlmsTxt {
            status: llms_file.as_ref().map(|response| response.status),
            title: None,
            links: 0,
            broken: Vec::new(),
        };
        if let Some(file) = llms_file.as_ref().filter(|_| llms.found()) {
            let text = String::from_utf8_lossy(&file.body);
            llms.title = llms::title(&text);
            let links = llms::links(&text, &llms_url);
            llms.links = links.len();
            let checked: Vec<_> = links.iter().take(CHECKED_LINKS).collect();
            let requests: Vec<_> = checked.iter().map(|link| get(link, &[], 1)).collect();
            for (link, answered) in checked.iter().zip(http::fetch_all(&requests)?) {
                if !answered.is_ok_and(|response| response.is_success()) {
                    llms.broken.push(link.to_string());
                }
            }
        }

        let (text, scripts) = content(&page);
        let markdown_suffix = suffix.is_some_and(|response| {
            response.is_success()
                && !response
                    .header("content-type")
                    .is_some_and(|kind| kind.contains("html"))
        });
        let markdown_accept = accept.is_some_and(|response| {
            response.is_success()
                && response
                    .header("content-type")
                    .is_some_and(|kind| kind.contains("markdown"))
        });

        let mut findings = Vec::new();
        for crawler in crawlers
            .iter()
            .filter(|crawler| crawler.rule == Rule::Blocked)
        {
            findings.push(Finding {
                code: Code::BlocksAiCrawler,
                level: Level::Warning,
                message: format!(
                    "robots.txt blocks {} from {} through the {} group.",
                    crawler.agent,
                    url.path(),
                    crawler.group.as_deref().unwrap_or("*")
                ),
            });
        }
        if !llms.found() {
            findings.push(Finding {
                code: Code::MissingLlmsTxt,
                level: Level::Info,
                message: "The site has no /llms.txt.".into(),
            });
        } else if llms.title.is_none() {
            findings.push(Finding {
                code: Code::InvalidLlmsTxt,
                level: Level::Warning,
                message: "/llms.txt does not open with an H1 title.".into(),
            });
        }
        for link in &llms.broken {
            findings.push(Finding {
                code: Code::BrokenLlmsTxtLink,
                level: Level::Warning,
                message: format!("/llms.txt links to {link}, which did not answer 2xx."),
            });
        }
        if text < SHELL_TEXT && scripts > 0 {
            findings.push(Finding {
                code: Code::ContentNeedsJavaScript,
                level: Level::Warning,
                message: format!(
                    "The HTML response holds {text} characters of text and {scripts} scripts; crawlers that do not run JavaScript see little content."
                ),
            });
        }
        if !markdown_suffix && !markdown_accept {
            findings.push(Finding {
                code: Code::NoMarkdownCopy,
                level: Level::Info,
                message: "The root page has no Markdown copy at its .md URL or for Accept: text/markdown.".into(),
            });
        }

        Ok(Self {
            url: url.to_string(),
            robots_status: robots.map(|response| response.status),
            crawlers,
            llms,
            text,
            scripts,
            markdown_suffix,
            markdown_accept,
            findings,
            page: report,
        })
    }
}

fn get<'a>(url: &'a Url, headers: &'a [(&'a str, &'a str)], limit: usize) -> http::Request<'a> {
    http::Request {
        method: "GET",
        url: url.as_str(),
        headers,
        body: &[],
        timeout: Some(TIMEOUT),
        limit: Some(limit),
    }
}

/// The page's Markdown copy as llmstxt.org names it: its URL with `.md`
/// appended, `index.html.md` for a path ending in `/`.
fn markdown_copy(url: &Url) -> Url {
    let mut copy = url.clone();
    copy.set_query(None);
    let path = if url.path().ends_with('/') {
        format!("{}index.html.md", url.path())
    } else {
        format!("{}.md", url.path())
    };
    copy.set_path(&path);
    copy
}

/// Characters of visible text in the body, and its script elements other than
/// JSON-LD.
fn content(page: &Page) -> (usize, usize) {
    let hidden = ["script", "style", "noscript", "template"];
    let text = page
        .document
        .select(&Selector::parse("body").expect("static selector"))
        .flat_map(|body| body.descendants())
        .filter_map(|node| match node.value() {
            Node::Text(text) => Some((node, text)),
            _ => None,
        })
        .filter(|(node, _)| {
            !node.ancestors().any(|ancestor| {
                ancestor
                    .value()
                    .as_element()
                    .is_some_and(|element| hidden.contains(&element.name()))
            })
        })
        .map(|(_, text)| {
            text.split_whitespace()
                .map(|word| word.chars().count())
                .sum::<usize>()
        })
        .sum();
    let scripts = page
        .document
        .select(
            &Selector::parse(r#"script:not([type="application/ld+json"])"#)
                .expect("static selector"),
        )
        .count();
    (text, scripts)
}
