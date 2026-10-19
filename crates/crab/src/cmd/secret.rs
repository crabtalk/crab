//! `crab secret` — a program's secrets on Cloud.

use crate::utils;
use anyhow::{Context, Result, bail};
use crabtalk_sdk::client::Client;
use std::io::Read;

/// Set `name`'s secret `secret` to the value on stdin, its trailing newline
/// dropped.
pub fn put(client: &Client, name: &str, secret: &str) -> Result<()> {
    let mut value = String::new();
    std::io::stdin()
        .read_to_string(&mut value)
        .context("reading the secret's value from stdin")?;
    let value = value.trim_end_matches(['\n', '\r']);
    if value.is_empty() {
        bail!("no value on stdin: `printf %s \"$VALUE\" | crab secret put {name} {secret}`");
    }
    utils::wait(client.put_secret(name, secret, value))?;
    eprintln!("set {name} {secret}");
    Ok(())
}

/// Print `name`'s secrets, by name.
pub fn ls(client: &Client, name: &str, json: bool) -> Result<()> {
    let secrets = utils::wait(client.secrets(name))?;
    if json {
        return utils::json(&secrets);
    }
    for secret in &secrets {
        println!("{}", secret.name);
    }
    Ok(())
}

/// Revoke `name`'s secret `secret`.
pub fn rm(client: &Client, name: &str, secret: &str, yes: bool) -> Result<()> {
    utils::confirm(yes, &format!("revoke {name} {secret}"))?;
    utils::wait(client.delete_secret(name, secret))?;
    eprintln!("revoked {name} {secret}");
    Ok(())
}
