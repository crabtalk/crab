use crate::{access::Access, report::Report};
use berm_lang::{Result, tool};
use webpage::page::Page;

/// Check page SEO
///
/// Inspect returned HTML and headers for SEO and social preview findings.
/// Does not render JavaScript, fetch linked resources or verify indexing.
#[tool(
    catalog = "web.seo",
    budget(cpu = 50000, read = 1, written = 1, memory = 23)
)]
pub fn check(
    /// Absolute HTTP or HTTPS URL, without embedded credentials.
    url: String,
) -> Result<Report> {
    Report::try_from(&Page::fetch(&url)?)
}

/// Check AI crawler access
///
/// Read a site's robots.txt rules for AI crawlers, its llms.txt, whether its
/// root page's content is in the HTML response, and whether that page has a
/// Markdown copy. Does not render JavaScript or crawl the site.
#[tool(
    catalog = "web.seo",
    budget(cpu = 90000, read = 1, written = 1, memory = 32)
)]
pub fn ai_access(
    /// Absolute HTTP or HTTPS URL of the site's root page, without embedded
    /// credentials.
    site: String,
) -> Result<Access> {
    Access::inspect(&site)
}
