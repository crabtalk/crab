//! A sitemap or a sitemap index, as sitemaps.org writes them.

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use xmlparser::{ElementEnd, Token, Tokenizer};

/// What one sitemap file lists.
#[derive(Default)]
pub struct Sitemap {
    /// The sitemaps an index names.
    pub sitemaps: Vec<String>,
    /// Each URL a urlset lists, with its `lastmod` as written.
    pub urls: Vec<(String, Option<String>)>,
    /// Whether a complete sitemap document was read.
    pub complete: bool,
}

impl Sitemap {
    /// The entries of `xml`. Elements are matched by their local name, an
    /// entry's `loc` and `lastmod` only as its own children, so the `loc` of
    /// an extension inside it, such as `image:image`, is not its; an entry
    /// with no `loc`, or one `xml` ends inside, is left out.
    pub fn parse(xml: &str) -> Self {
        let mut sitemap = Sitemap::default();
        // The entry being read, `sitemap` or `url`, its `loc` and `lastmod`.
        let mut entry: Option<(bool, String, Option<String>)> = None;
        // The child of the entry whose text is being read: true for `loc`.
        let mut reading: Option<bool> = None;
        let mut open = "";
        // How deep the element open is, and the depth of the entry's.
        let (mut depth, mut within) = (0, 0);
        let mut root = None;
        let mut names = Vec::new();
        let mut valid = true;
        for token in Tokenizer::from(xml) {
            let Ok(token) = token else {
                sitemap.complete = false;
                break;
            };
            match token {
                Token::ElementStart { prefix, local, .. } => {
                    open = local.as_str();
                    names.push((prefix.as_str(), open));
                    if depth == 0 {
                        root = Some(open);
                    }
                    depth += 1;
                    let child = entry.is_some() && depth == within + 1;
                    match open {
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
                        ElementEnd::Close(prefix, local) => {
                            valid &= names.pop() == Some((prefix.as_str(), local.as_str()));
                            local.as_str()
                        }
                        ElementEnd::Empty => {
                            names.pop();
                            open
                        }
                        ElementEnd::Open => continue,
                    };
                    depth -= 1;
                    if depth == 0 {
                        sitemap.complete = valid
                            && root == Some(closed)
                            && matches!(closed, "urlset" | "sitemapindex");
                    }
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
