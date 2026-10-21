//! A sitemap or a sitemap index, as sitemaps.org writes them.

use alloc::{string::String, vec::Vec};
use xmlparser::{ElementEnd, Token, Tokenizer};

/// What one sitemap file lists.
#[derive(Default)]
pub struct Sitemap {
    /// The sitemaps an index names.
    pub sitemaps: Vec<String>,
    /// Each URL a urlset lists, with its `lastmod` as written.
    pub urls: Vec<(String, Option<String>)>,
    /// Whether it lists more URLs than it was read for.
    pub more: bool,
}

impl Sitemap {
    /// The entries of `xml`, its first `limit` URLs at most. Elements are matched by their local name, an
    /// entry's `loc` and `lastmod` only as its own children, so the `loc` of
    /// an extension inside it, such as `image:image`, is not its; an entry
    /// with no `loc` is left out.
    pub fn parse(xml: &str, limit: usize) -> Self {
        let mut sitemap = Sitemap::default();
        // The entry being read, `sitemap` or `url`, its `loc` and `lastmod`.
        let mut entry: Option<(bool, String, Option<String>)> = None;
        // The child of the entry whose text is being read: true for `loc`.
        let mut reading: Option<bool> = None;
        let mut open = "";
        // How deep the element open is, and the depth of the entry's.
        let (mut depth, mut within) = (0, 0);
        for token in Tokenizer::from(xml).flatten() {
            match token {
                Token::ElementStart { local, .. } => {
                    open = local.as_str();
                    depth += 1;
                    let child = entry.is_some() && depth == within + 1;
                    match open {
                        "url" if sitemap.urls.len() == limit => {
                            sitemap.more = true;
                            break;
                        }
                        "sitemap" | "url" => {
                            entry = Some((open == "sitemap", String::new(), None));
                            within = depth;
                        }
                        "loc" if child => reading = Some(true),
                        "lastmod" if child => reading = Some(false),
                        _ => {}
                    }
                }
                Token::Text { text } | Token::Cdata { text, .. } => {
                    let Some((_, loc, lastmod)) = entry.as_mut() else {
                        continue;
                    };
                    match reading {
                        Some(true) => loc.push_str(&html::unescape(text.as_str())),
                        Some(false) => lastmod
                            .get_or_insert_with(String::new)
                            .push_str(text.as_str()),
                        None => {}
                    }
                }
                Token::ElementEnd { end, .. } => {
                    let closed = match end {
                        ElementEnd::Close(_, local) => local.as_str(),
                        ElementEnd::Empty => open,
                        ElementEnd::Open => continue,
                    };
                    depth -= 1;
                    match closed {
                        "loc" | "lastmod" => reading = None,
                        "sitemap" | "url" => {
                            let Some((index, loc, lastmod)) = entry.take() else {
                                continue;
                            };
                            let loc = String::from(loc.trim());
                            if loc.is_empty() {
                                continue;
                            }
                            match index {
                                true => sitemap.sitemaps.push(loc),
                                false => sitemap.urls.push((
                                    loc,
                                    lastmod.map(|lastmod| String::from(lastmod.trim())),
                                )),
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        sitemap
    }
}
