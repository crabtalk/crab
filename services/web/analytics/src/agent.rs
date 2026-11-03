//! Who a view came from: an AI crawler by its user agent, an AI assistant by
//! the host it was referred from.

/// User agent tokens of AI crawlers and the fetchers AI assistants send on a
/// user's behalf.
const CRAWLERS: &[&str] = &[
    "GPTBot",
    "OAI-SearchBot",
    "ChatGPT-User",
    "ClaudeBot",
    "Claude-User",
    "Claude-SearchBot",
    "PerplexityBot",
    "Perplexity-User",
    "MistralAI-User",
    "DuckAssistBot",
    "meta-externalagent",
    "Amazonbot",
    "Bytespider",
    "CCBot",
];

/// Hosts of AI assistants that link to the pages they cite.
pub(crate) const ASSISTANTS: &[&str] = &[
    "chatgpt.com",
    "chat.openai.com",
    "perplexity.ai",
    "www.perplexity.ai",
    "claude.ai",
    "gemini.google.com",
    "copilot.microsoft.com",
    "chat.deepseek.com",
    "chat.mistral.ai",
    "grok.com",
    "meta.ai",
    "www.meta.ai",
];

/// The crawler `agent` names, by its token.
pub fn crawler(agent: &str) -> Option<&'static str> {
    CRAWLERS.iter().copied().find(|token| agent.contains(token))
}

/// Whether `host` is an AI assistant's.
pub fn assistant(host: &str) -> bool {
    ASSISTANTS.contains(&host)
}
