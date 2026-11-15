use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Service-authored review and the immutable input to its action tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review<T> {
    pub title: String,
    pub body: String,
    pub tool: String,
    pub input: T,
}

impl<T> Review<T> {
    pub fn check(&self) -> Result<(), &'static str> {
        if self.title.trim().is_empty()
            || self.title.len() > 160
            || self.title.chars().any(char::is_control)
        {
            return Err("A request title is 1–160 bytes on one line");
        }
        if self.body.trim().is_empty() || self.body.len() > 8000 {
            return Err("A request review is 1–8000 bytes of Markdown");
        }
        if self.tool.is_empty()
            || self.tool.len() > 100
            || !self
                .tool
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("A request action names one tool in the originating service");
        }
        Ok(())
    }
}

/// The host supplies the same request ID and input on every execution retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution<T> {
    pub request: String,
    pub input: T,
}
