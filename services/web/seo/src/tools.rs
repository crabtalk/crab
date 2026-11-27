use crate::{access::Access, report::Report};
use berm_lang::{Result, tool};
use webpage::page::Page;

/// Check page SEO
///
/// Inspect returned HTML and headers for SEO and social preview findings.
/// Does not render JavaScript, fetch linked resources or verify indexing.
#[tool(
    catalog = "web.seo",
    example = "check https://example.com for technical SEO issues and list what to fix first",
    example = "check whether the title, description and social preview on my landing page are set",
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
/// Read robots.txt rules and Content Signals for AI crawlers, check llms.txt,
/// and inspect the root page for readable HTML, discovery links and Markdown.
#[tool(
    catalog = "web.seo",
    example = "check whether AI crawlers like GPTBot and ClaudeBot can read https://example.com",
    example = "tell me if my site publishes llms.txt and Markdown pages for AI",
    budget(cpu = 90000, read = 1, written = 1, memory = 32)
)]
pub fn ai_access(
    /// Absolute HTTP or HTTPS URL of the site's root page, without embedded
    /// credentials.
    site: String,
) -> Result<Access> {
    Access::inspect(&site)
}
