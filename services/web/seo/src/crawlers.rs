//! `robots.txt` rules for the crawlers AI products send, read as RFC 9309
//! reads them.

use berm_lang::Schema;

/// The user-agent tokens checked, as each product documents them.
pub const AGENTS: [&str; 11] = [
    "GPTBot",
    "OAI-SearchBot",
    "ChatGPT-User",
    "ClaudeBot",
    "Claude-SearchBot",
    "Claude-User",
    "PerplexityBot",
    "Perplexity-User",
    "Google-Extended",
    "Applebot-Extended",
    "CCBot",
];

#[derive(Debug, PartialEq, Eq, Schema)]
pub enum Rule {
    Allowed,
    Blocked,
    /// No group names the crawler and there is no `*` group.
    Unspecified,
}

#[derive(Debug, Schema)]
pub struct Crawler {
    pub agent: String,
    pub rule: Rule,
    /// The user-agent of the group that applied: the crawler's own, or `*`.
    pub group: Option<String>,
}

struct Group {
    agents: Vec<String>,
    /// `(allow, path pattern)` in file order.
    rules: Vec<(bool, String)>,
}

/// A parsed `robots.txt`.
pub struct Robots {
    groups: Vec<Group>,
}

impl From<&str> for Robots {
    fn from(text: &str) -> Self {
        let mut groups: Vec<Group> = Vec::new();
        let mut open = false;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default().trim();
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "user-agent" => {
                    if !open {
                        groups.push(Group {
                            agents: Vec::new(),
                            rules: Vec::new(),
                        });
                        open = true;
                    }
                    if let Some(group) = groups.last_mut() {
                        group.agents.push(value.to_owned());
                    }
                }
                rule @ ("allow" | "disallow") => {
                    open = false;
                    if let Some(group) = groups.last_mut()
                        && !value.is_empty()
                    {
                        group.rules.push((rule == "allow", value.to_owned()));
                    }
                }
                _ => {}
            }
        }
        Self { groups }
    }
}

impl Robots {
    /// What applies to `agent` fetching `path`: its own groups, else `*`'s;
    /// the longest matching pattern wins, and `allow` wins a tie.
    pub fn crawler(&self, agent: &str, path: &str) -> Crawler {
        let named = |name: &str| {
            self.groups
                .iter()
                .filter(|group| {
                    group
                        .agents
                        .iter()
                        .any(|given| given.eq_ignore_ascii_case(name))
                })
                .collect::<Vec<_>>()
        };
        let (groups, group) = match named(agent) {
            own if !own.is_empty() => (own, agent),
            _ => (named("*"), "*"),
        };
        if groups.is_empty() {
            return Crawler {
                agent: agent.into(),
                rule: Rule::Unspecified,
                group: None,
            };
        }
        let best = groups
            .iter()
            .flat_map(|group| &group.rules)
            .filter(|(_, pattern)| matches(pattern, path))
            .max_by_key(|(allow, pattern)| (pattern.len(), *allow));
        Crawler {
            agent: agent.into(),
            rule: match best {
                Some((false, _)) => Rule::Blocked,
                _ => Rule::Allowed,
            },
            group: Some(group.into()),
        }
    }
}

/// Whether `path` matches `pattern`, where `*` is any run of characters and a
/// final `$` ends the path.
fn matches(pattern: &str, path: &str) -> bool {
    let (pattern, anchored) = match pattern.strip_suffix('$') {
        Some(pattern) => (pattern, true),
        None => (pattern, false),
    };
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = path.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<_> = parts.collect();
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        if last && anchored {
            return rest.ends_with(part);
        }
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    !anchored || rest.is_empty()
}
