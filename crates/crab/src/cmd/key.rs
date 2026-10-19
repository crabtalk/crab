//! `crab key` — the account's API keys, which deploy from CI.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::{
    account::key::{CreateKeyRequest, Scope},
    client::Client,
};

pub fn ls(client: &Client, json: bool) -> Result<()> {
    let keys = utils::wait(client.keys())?;
    if json {
        return utils::json(&keys);
    }
    for key in &keys {
        let state = match (key.revoked_at, key.expires_at) {
            (Some(at), _) => format!("revoked {}", format::date(at)),
            (None, Some(at)) => format!("expires {}", format::date(at)),
            (None, None) => "active".to_owned(),
        };
        println!(
            "{:<26}  {:<24}  {:<10}  {state}",
            key.id, key.label, key.hint
        );
    }
    Ok(())
}

/// Mint a deploy key named `label`, expiring after `days` when given. The
/// token is printed once.
pub fn create(client: &Client, label: &str, days: Option<u32>, json: bool) -> Result<()> {
    let expires_at = days.map(|days| chrono::Utc::now().timestamp() + i64::from(days) * 86_400);
    let created = utils::wait(client.create_key(&CreateKeyRequest {
        label: label.to_owned(),
        scope: vec![Scope::Deploy],
        expires_at,
    }))?;
    if json {
        return utils::json(&created);
    }
    println!("{}", created.token);
    eprintln!("key {} made; the token above is shown once", created.key.id);
    Ok(())
}

pub fn rm(client: &Client, id: &str, yes: bool) -> Result<()> {
    utils::confirm(yes, &format!("revoke key {id}"))?;
    utils::wait(client.delete_key(id))?;
    eprintln!("revoked {id}");
    Ok(())
}
