use alloc::string::String;
use berm_lang::{Schema, serde_json::Value};

/// A pipeline or transaction result in command order. Null means no value.
#[derive(Debug, Clone, Schema)]
pub struct CommandResult {
    pub value: Value,
    pub error: Option<String>,
}
