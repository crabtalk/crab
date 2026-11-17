use crate::report::Report;
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
