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
}

impl Sitemap {
    /// The entries of `xml`. Elements are matched by their local name; an
    /// entry with no `loc` is left out.
    pub fn parse(xml: &str) -> Self {
        let mut sitemap = Sitemap::default();
        // The entry being read, `sitemap` or `url`, its `loc` and `lastmod`.
        let mut entry: Option<(bool, String, Option<String>)> = None;
        // The child of the entry whose text is being read: true for `loc`.
        let mut reading: Option<bool> = None;
        let mut open = "";
        for token in Tokenizer::from(xml).flatten() {
            match token {
                Token::ElementStart { local, .. } => {
                    open = local.as_str();
                    match open {
                        "sitemap" => entry = Some((true, String::new(), None)),
                        "url" => entry = Some((false, String::new(), None)),
                        "loc" if entry.is_some() => reading = Some(true),
                        "lastmod" if entry.is_some() => reading = Some(false),
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
