use crate::{page::Page, reading::Reading};
use berm_lang::{Result, tool};

/// Read webpage
///
/// Read a public HTML page as Markdown, with headings, links, lists and code.
/// Fetches server HTML without executing JavaScript.
#[tool(
    catalog = "web.scrape",
    budget(cpu = 60000, read = 1, written = 1, memory = 12)
)]
pub fn read(
    /// Absolute HTTP or HTTPS URL, without embedded credentials.
    url: String,
) -> Result<Reading> {
    Reading::try_from(&Page::fetch(&url)?)
}
