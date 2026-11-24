//! `robots.txt` rules for the crawlers search engines and AI products send,
//! read as RFC 9309 reads them.

use berm_lang::Schema;
use std::collections::BTreeMap;

/// The user-agent tokens checked, as each product documents them.
pub const AGENTS: [&str; 22] = [
    "GPTBot",
    "OAI-SearchBot",
    "ChatGPT-User",
    "ClaudeBot",
    "Claude-SearchBot",
    "Claude-User",
    "PerplexityBot",
    "Perplexity-User",
    "Googlebot",
    "Google-Extended",
    "Google-CloudVertexBot",
    "bingbot",
    "Applebot",
    "Applebot-Extended",
    "Bytespider",
    "meta-externalagent",
    "meta-externalfetcher",
    "FacebookBot",
    "Amazonbot",
    "DuckAssistBot",
    "MistralAI-User",
    "CCBot",
];

#[derive(Debug, PartialEq, Eq, Schema)]
pub enum Rule {
    Allowed,
    Blocked,
    /// No group names the crawler and there is no `*` group.
    Unspecified,
}

#[derive(Debug, Default, PartialEq, Eq, Schema)]
pub struct ContentSignals {
    pub search: Option<bool>,
    pub ai_input: Option<bool>,
    pub ai_train: Option<bool>,
}

impl ContentSignals {
    fn read(&mut self, value: &str) {
        for pair in value.split(',') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let signal = match key.trim().to_ascii_lowercase().as_str() {
                "search" => &mut self.search,
                "ai-input" => &mut self.ai_input,
                "ai-train" => &mut self.ai_train,
                _ => continue,
            };
            let value = value.trim();
            if value.eq_ignore_ascii_case("no") {
                *signal = Some(false);
            } else if value.eq_ignore_ascii_case("yes") {
                signal.get_or_insert(true);
            }
        }
    }

    fn merge(&mut self, other: &Self) {
        for (signal, value) in [
            (&mut self.search, other.search),
            (&mut self.ai_input, other.ai_input),
            (&mut self.ai_train, other.ai_train),
        ] {
            if let Some(value) = value {
                *signal = Some(signal.unwrap_or(true) && value);
            }
        }
    }
}

#[derive(Debug, Schema)]
pub struct Crawler {
    pub agent: String,
    pub rule: Rule,
    /// The user-agent of the group that applied: the crawler's own, or `*`.
    pub group: Option<String>,
    pub content_signals: ContentSignals,
}

pub(crate) struct Group {
    pub agents: Vec<String>,
    /// `(allow, path pattern)` in file order.
    rules: Vec<(bool, String)>,
    pub content_signals: ContentSignals,
}

impl Group {
    fn names(&self, agent: &str) -> bool {
        self.agents
            .iter()
            .any(|given| given.eq_ignore_ascii_case(agent))
    }
}

/// A parsed `robots.txt`.
pub struct Robots {
    groups: Vec<Group>,
    pub(crate) sitemaps: Vec<String>,
}

impl From<&str> for Robots {
    fn from(text: &str) -> Self {
        let mut groups: Vec<Group> = Vec::new();
        let mut open = false;
        let mut sitemaps = Vec::new();
        for line in text.trim_start_matches('\u{feff}').lines() {
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
                            content_signals: ContentSignals::default(),
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
                "content-signal" => {
                    if let Some(group) = groups.last_mut() {
                        group.content_signals.read(value);
                    }
                }
                "sitemap" if !value.is_empty() => sitemaps.push(value.to_owned()),
                _ => {}
            }
        }
        Self { groups, sitemaps }
    }
}

impl Robots {
    fn groups_for<'a>(&self, agent: &'a str) -> (&'a str, Vec<(usize, &Group)>) {
        let named = |name| {
            self.groups
                .iter()
                .enumerate()
                .filter(|(_, group)| group.names(name))
                .collect::<Vec<_>>()
        };
        match named(agent) {
            own if !own.is_empty() => (agent, own),
            _ => ("*", named("*")),
        }
    }

    pub(crate) fn restrictive_groups(&self) -> impl Iterator<Item = (&Group, Vec<&'static str>)> {
        let mut applied = BTreeMap::<_, Vec<_>>::new();
        for agent in AGENTS {
            for (index, group) in self.groups_for(agent).1 {
                if group.content_signals.ai_input == Some(false)
                    || group.content_signals.search == Some(false)
                {
                    applied.entry(index).or_default().push(agent);
                }
            }
        }
        applied
            .into_iter()
            .map(|(index, agents)| (&self.groups[index], agents))
    }

    /// What applies to `agent` fetching `path`: its own groups, else `*`'s;
    /// the longest matching pattern wins, and `allow` wins a tie.
    pub fn crawler(&self, agent: &str, path: &str) -> Crawler {
        let (group, groups) = self.groups_for(agent);
        if groups.is_empty() {
            return Crawler {
                agent: agent.into(),
                rule: Rule::Unspecified,
                group: None,
                content_signals: ContentSignals::default(),
            };
        }
        let best = groups
            .iter()
            .flat_map(|(_, group)| &group.rules)
            .filter(|(_, pattern)| matches(pattern, path))
            .max_by_key(|(allow, pattern)| (pattern.len(), *allow));
        let mut content_signals = ContentSignals::default();
        for (_, group) in groups {
            content_signals.merge(&group.content_signals);
        }
        Crawler {
            agent: agent.into(),
            rule: match best {
                Some((false, _)) => Rule::Blocked,
                _ => Rule::Allowed,
            },
            group: Some(group.into()),
            content_signals,
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
