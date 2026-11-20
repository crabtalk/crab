use crate::{
    audit::discovery::{request, web_url},
    report::{Code, Report},
};
use berm_lang::{Result, Schema, http, tool};
use scraper::Selector;
use std::collections::BTreeSet;
use url::Url;
use webpage::page::{MAX_HTML, Page};

const MAX_LINKS: usize = 8;
const MAX_METADATA_BYTES: usize = 512;

#[derive(Clone, Schema)]
pub(crate) struct PageScan {
    pub url: String,
    pub status: Option<u16>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical: Option<String>,
    pub links: Vec<String>,
    pub links_truncated: bool,
    pub metadata: Vec<Code>,
    pub problem: Option<String>,
}

/// Inspect a sitemap page
///
/// Collects page findings and a bounded sample of links for the site report.
#[tool(
    query,
    access = "internal",
    budget(cpu = 100000, read = 1, written = 1, memory = 32)
)]
pub(crate) fn audit_page(url: String) -> Result<PageScan> {
    let target = Url::parse(&url)
        .ok()
        .and_then(web_url)
        .ok_or("Invalid sitemap page URL")?;
    let mut scan = PageScan {
        url,
        status: None,
        title: None,
        description: None,
        canonical: None,
        links: Vec::new(),
        links_truncated: false,
        metadata: Vec::new(),
        problem: None,
    };
    let response = http::fetch_all(&[request(target.as_str(), MAX_HTML + 1)])?
        .pop()
        .and_then(std::result::Result::ok);
    let Some(response) = response else {
        scan.problem = Some("The page did not answer".into());
        return Ok(scan);
    };
    scan.status = Some(response.status);
    let page = match Page::try_from((target, response)) {
        Ok(page) => page,
        Err(error) => {
            scan.problem = Some(error.message);
            return Ok(scan);
        }
    };
    let report = match Report::try_from(&page) {
        Ok(report) => report,
        Err(error) => {
            scan.problem = Some(error.message);
            return Ok(scan);
        }
    };
    scan.title = report.title.filter(|text| text.len() <= MAX_METADATA_BYTES);
    scan.description = report
        .description
        .filter(|text| text.len() <= MAX_METADATA_BYTES);
    scan.canonical = report
        .canonical
        .and_then(|url| Url::parse(&url).ok())
        .and_then(web_url)
        .map(Into::into);
    scan.metadata = report
        .findings
        .into_iter()
        .map(|finding| finding.code)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let selector = Selector::parse("a[href]").expect("static selector");
    let mut seen = BTreeSet::new();
    for link in page.document.select(&selector).filter_map(|element| {
        page.base
            .join(element.value().attr("href")?)
            .ok()
            .and_then(web_url)
    }) {
        if link.origin() != page.url.origin() || link == page.url || !seen.insert(link.clone()) {
            continue;
        }
        if scan.links.len() == MAX_LINKS {
            scan.links_truncated = true;
            break;
        }
        scan.links.push(link.into());
    }
    Ok(scan)
}
