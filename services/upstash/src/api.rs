use crate::types::CommandResult;
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use berm_lang::{
    Result,
    http::Client,
    serde::Serialize,
    serde_json::{self, Value},
};
use url::Url;

/// A database's HTTPS REST endpoint. Credentials remain in the host.
pub struct Redis {
    endpoint: String,
}
impl Redis {
    /// Accept an Upstash database origin, without credentials, path or query.
    pub fn new(endpoint: &str) -> Result<Self> {
        let url = Url::parse(endpoint).map_err(|_| "Invalid Upstash REST URL")?;
        if url.scheme() != "https"
            || !url.host_str().is_some_and(|h| h.ends_with(".upstash.io"))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "Use the HTTPS database origin from Upstash, without a path or credentials".into(),
            );
        }
        Ok(Self {
            endpoint: url.as_str().trim_end_matches('/').into(),
        })
    }
    fn request(&self, path: &str, body: &impl Serialize) -> Result<Value> {
        let body = serde_json::to_vec(body)?;
        let response = Client {
            name: "Upstash",
            headers: &[
                ("Authorization", "Bearer {secret:UPSTASH_REDIS_REST_TOKEN}"),
                ("Content-Type", "application/json"),
            ],
        }
        .request("POST", &format!("{}{path}", self.endpoint), &body)?;
        let value: Value = response.read("Upstash")?;
        if value["error"].is_string() {
            return Err(response.failure("Upstash"));
        }
        Ok(value)
    }
    /// Execute a Redis command. Arguments are strings; responses preserve JSON types.
    pub fn command(&self, command: &[String]) -> Result<Value> {
        if command.is_empty() {
            return Err("A Redis command must not be empty".into());
        }
        let value = self.request("", &command)?;
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "Upstash response has no result".into())
    }
    /// Read a string; a missing key returns None.
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        serde_json::from_value(self.command(&["GET".into(), key.into()])?)
            .map_err(|_| "Upstash GET returned a non-string value".into())
    }
    /// Write a string, optionally expiring it after a positive number of seconds.
    pub fn set(&self, key: &str, value: &str, expiry_seconds: Option<u64>) -> Result<()> {
        let mut command = vec!["SET".into(), key.into(), value.into()];
        if let Some(seconds) = expiry_seconds {
            if seconds == 0 {
                return Err("Expiry must be greater than zero".into());
            }
            command.extend(["EX".into(), seconds.to_string()]);
        }
        if self.command(&command)? != "OK" {
            return Err("Upstash SET did not return OK".into());
        }
        Ok(())
    }
    /// Delete keys and return how many existed.
    pub fn delete(&self, keys: &[String]) -> Result<u64> {
        if keys.is_empty() {
            return Err("Provide at least one key".into());
        }
        let mut command = vec!["DEL".into()];
        command.extend_from_slice(keys);
        self.command(&command)?
            .as_u64()
            .ok_or_else(|| "Upstash DEL returned a non-integer".into())
    }
    /// Set a key's expiry. Zero deletes it immediately; false means it did not exist.
    pub fn expire(&self, key: &str, seconds: u64) -> Result<bool> {
        match self
            .command(&["EXPIRE".into(), key.into(), seconds.to_string()])?
            .as_u64()
        {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => Err("Upstash EXPIRE returned an invalid result".into()),
        }
    }
    /// Remaining seconds: -1 means persistent, -2 means missing.
    pub fn ttl(&self, key: &str) -> Result<i64> {
        self.command(&["TTL".into(), key.into()])?
            .as_i64()
            .ok_or_else(|| "Upstash TTL returned a non-integer".into())
    }
    /// Increment by a signed amount. Failures are never automatically retried.
    pub fn increment(&self, key: &str, amount: i64) -> Result<i64> {
        self.command(&["INCRBY".into(), key.into(), amount.to_string()])?
            .as_i64()
            .ok_or_else(|| "Upstash INCRBY returned a non-integer".into())
    }
    fn batch(&self, path: &str, commands: &[Vec<String>]) -> Result<Vec<CommandResult>> {
        if commands.is_empty() || commands.iter().any(Vec::is_empty) {
            return Err("Provide nonempty Redis commands".into());
        }
        let value = self.request(path, &commands)?;
        let rows = value
            .as_array()
            .filter(|rows| rows.len() == commands.len())
            .ok_or("Upstash returned an invalid batch response")?;
        rows.iter()
            .map(|row| {
                let error = row.get("error").and_then(Value::as_str).map(String::from);
                if error.is_none() && row.get("result").is_none() {
                    return Err("Upstash batch item has no result or error".into());
                }
                Ok(CommandResult {
                    value: row.get("result").cloned().unwrap_or(Value::Null),
                    error,
                })
            })
            .collect()
    }
    /// Execute in order with possible interleaving from other clients. Inspect each error.
    pub fn pipeline(&self, commands: &[Vec<String>]) -> Result<Vec<CommandResult>> {
        self.batch("/pipeline", commands)
    }
    /// Execute without interleaving. Runtime command errors do not roll back other commands.
    pub fn transaction(&self, commands: &[Vec<String>]) -> Result<Vec<CommandResult>> {
        self.batch("/multi-exec", commands)
    }
}
