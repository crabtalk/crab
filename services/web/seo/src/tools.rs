use crate::report::Report;
use berm_lang::{Result, tool};
use webpage::page::Page;

/// Inspect server-returned HTML for basic SEO findings and their evidence.
/// Checks page metadata, canonical links, h1 headings, image alt attributes
/// and robots directives. It does not measure rankings or render JavaScript.
#[tool]
pub fn check(
    /// Absolute HTTP or HTTPS URL, without embedded credentials.
    url: String,
) -> Result<Report> {
    Report::try_from(&Page::fetch(&url)?)
}
