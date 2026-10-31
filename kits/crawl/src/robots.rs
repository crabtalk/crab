//! A robots.txt, read as RFC 9309 says: the group for a crawler's token, else
//! the group for `*`, its rules matched longest first, `allow` winning a tie.

use alloc::{borrow::Cow, string::String, vec::Vec};
use url::{Position, Url};

/// What a crawler reads out of a robots.txt.
#[derive(Default)]
pub struct Robots {
    /// Each rule of the crawler's group: whether it allows, and its path
    /// pattern, [`encoded`].
    rules: Vec<(bool, String)>,
    /// The `Sitemap:` URLs, in the order written.
    pub sitemaps: Vec<String>,
    /// The first `Content-Signal:` value, as written.
    pub signal: Option<String>,
}

impl Robots {
    /// `text` as read by the crawler whose `User-Agent` is `agent`, its group
    /// matched by the part of `agent` before the first `/`.
    pub fn parse(text: &str, agent: &str) -> Self {
        let token = agent.split('/').next().unwrap_or(agent);
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
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
                let rule = (is("allow"), encoded(value).into_owned());
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

    /// Whether the crawler may fetch `url`, matched by its path and query.
    pub fn allows(&self, url: &Url) -> bool {
        let path = encoded(&url[Position::BeforePath..Position::AfterQuery]);
        self.rules
            .iter()
            .filter(|(_, pattern)| matches(pattern.as_bytes(), path.as_bytes()))
            .max_by_key(|(allow, pattern)| (pattern.len(), *allow))
            .is_none_or(|(allow, _)| *allow)
    }
}

/// `text` with each octet outside ASCII percent-encoded and the hex digits of
/// each `%XX` already in it uppercased, as RFC 9309 §2.2.2 compares paths.
fn encoded(text: &str) -> Cow<'_, str> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let bytes = text.as_bytes();
    if bytes.iter().all(|&byte| byte.is_ascii() && byte != b'%') {
        return Cow::Borrowed(text);
    }
    let mut encoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            0x80.. => encoded.extend([
                b'%',
                HEX[usize::from(byte >> 4)],
                HEX[usize::from(byte & 15)],
            ]),
            b'%' if bytes
                .get(at + 1..at + 3)
                .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit)) =>
            {
                encoded.extend([
                    b'%',
                    bytes[at + 1].to_ascii_uppercase(),
                    bytes[at + 2].to_ascii_uppercase(),
                ]);
                at += 2;
            }
            _ => encoded.push(byte),
        }
        at += 1;
    }
    // Every byte pushed is ASCII.
    Cow::Owned(String::from_utf8(encoded).unwrap_or_default())
}

/// Whether `pattern`, with `*` for any run and a final `$` for the end,
/// matches the start of `path`.
fn matches(pattern: &[u8], path: &[u8]) -> bool {
    let (pattern, anchored) = match pattern.strip_suffix(b"$") {
        Some(pattern) => (pattern, true),
        None => (pattern, false),
    };
    let (mut p, mut s) = (0, 0);
    // Where to resume after the last `*`: the pattern past it, and the path
    // position it has taken up to.
    let mut star = None;
    loop {
        match pattern.get(p) {
            None if !anchored || s == path.len() => return true,
            Some(b'*') => {
                p += 1;
                star = Some((p, s));
                continue;
            }
            Some(&byte) if path.get(s) == Some(&byte) => {
                p += 1;
                s += 1;
                continue;
            }
            _ => {}
        }
        match star {
            Some((after, taken)) if taken < path.len() => {
                star = Some((after, taken + 1));
                (p, s) = (after, taken + 1);
            }
            _ => return false,
        }
    }
}
