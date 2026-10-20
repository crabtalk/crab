//! A robots.txt, read as RFC 9309 says: the group for a crawler's token, else
//! the group for `*`, its rules matched longest first, `allow` winning a tie.

use alloc::{string::String, vec::Vec};

/// What a crawler reads out of a robots.txt.
#[derive(Default)]
pub struct Robots {
    /// Each rule of the crawler's group: whether it allows, and its path
    /// pattern.
    rules: Vec<(bool, String)>,
    /// The `Sitemap:` URLs, in the order written.
    pub sitemaps: Vec<String>,
    /// The first `Content-Signal:` value, as written.
    pub signal: Option<String>,
}

impl Robots {
    /// `text` as read by the crawler whose token is `token`.
    pub fn parse(text: &str, token: &str) -> Self {
        let mut robots = Robots::default();
        let (mut named, mut star) = (Vec::new(), Vec::new());
        // The agents of the group being read, and whether its rules began.
        let (mut agents, mut ruled) = (Vec::<String>::new(), false);
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default();
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let is = |name: &str| key.eq_ignore_ascii_case(name);
            if is("sitemap") {
                robots.sitemaps.push(String::from(value));
            } else if is("content-signal") {
                robots.signal.get_or_insert_with(|| String::from(value));
            } else if is("user-agent") {
                if ruled {
                    agents.clear();
                    ruled = false;
                }
                agents.push(value.to_ascii_lowercase());
            } else if is("allow") || is("disallow") {
                ruled = true;
                if value.is_empty() {
                    continue;
                }
                let rule = (is("allow"), String::from(value));
                if agents.iter().any(|agent| agent.eq_ignore_ascii_case(token)) {
                    named.push(rule.clone());
                }
                if agents.iter().any(|agent| agent == "*") {
                    star.push(rule);
                }
            }
        }
        robots.rules = if named.is_empty() { star } else { named };
        robots
    }

    /// Whether the crawler may fetch `path`, the path and query of a URL.
    pub fn allows(&self, path: &str) -> bool {
        self.rules
            .iter()
            .filter(|(_, pattern)| matches(pattern.as_bytes(), path.as_bytes()))
            .max_by_key(|(allow, pattern)| (pattern.len(), *allow))
            .is_none_or(|(allow, _)| *allow)
    }
}

/// Whether `pattern`, with `*` for any run and a final `$` for the end,
/// matches the start of `path`.
fn matches(pattern: &[u8], path: &[u8]) -> bool {
    match pattern.split_first() {
        None => true,
        Some((b'$', [])) => path.is_empty(),
        Some((b'*', rest)) => (0..=path.len()).any(|skip| matches(rest, &path[skip..])),
        Some((&byte, rest)) => path.first() == Some(&byte) && matches(rest, &path[1..]),
    }
}
