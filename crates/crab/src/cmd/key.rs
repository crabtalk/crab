//! `crab key` — the account's API keys, which deploy from CI and call other
//! accounts' priced services within a daily budget.

use crate::{cmd::request, format, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::{
    account::key::{CreateKeyRequest, Scope, UpdateKeyRequest},
    client::Client,
};

pub fn ls(client: &Client, json: bool) -> Result<()> {
    let keys = runtime::wait(client.keys())?;
    if json {
        return terminal::json(&keys);
    }
    for key in &keys {
        let state = match (key.revoked_at, key.expires_at) {
            (Some(at), _) => format!("revoked {}", format::date(at)),
            (None, Some(at)) => format!("expires {}", format::date(at)),
            (None, None) => "active".to_owned(),
        };
        println!(
            "{:<26}  {:<24}  {:<10}  {} of {} today  {state}",
            key.id,
            key.label,
            key.hint,
            format::usd(key.spent as f64),
            format::usd(key.budget as f64)
        );
        for granted in &key.services {
            println!("  may call {}", granted.service);
        }
    }
    Ok(())
}

/// Mint a deploy key named `label`, expiring after `days` when given. The
/// token is printed once.
pub fn create(client: &Client, label: &str, days: Option<u32>, json: bool) -> Result<()> {
    let expires_at = days.map(|days| chrono::Utc::now().timestamp() + i64::from(days) * 86_400);
    let created = runtime::wait(client.create_key(&CreateKeyRequest {
        label: label.to_owned(),
        scope: vec![Scope::Deploy],
        expires_at,
        budget: None,
    }))?;
    if json {
        return terminal::json(&created);
    }
    println!("{}", created.token);
    eprintln!("key {} made; the token above is shown once", created.key.id);
    Ok(())
}

pub fn rm(client: &Client, id: &str, yes: bool) -> Result<()> {
    terminal::confirm(yes, &format!("revoke key {id}"))?;
    runtime::wait(client.delete_key(id))?;
    eprintln!("revoked {id}");
    Ok(())
}

/// Ask an owner of the account to let the key `crab` signs in with call
/// `services`, each `owner/name`, or spend `budget` USD a UTC day: print its
/// page, and how to wait for it.
pub fn ask(
    client: &Client,
    services: &[String],
    budget: Option<u32>,
    context: &str,
    json: bool,
) -> Result<()> {
    let budget = budget.map(|usd| i64::from(usd) * 1_000_000);
    let asked = runtime::wait(client.ask_grant(services.to_vec(), budget, context))?;
    request::asked(&asked, json)
}

/// Let key `id` call `service`, `owner/name`.
pub fn grant(client: &Client, id: &str, service: &str) -> Result<()> {
    runtime::wait(client.grant_key(id, service))?;
    eprintln!("key {id} may call {service}");
    Ok(())
}

/// Stop key `id` calling `service`, `owner/name`.
pub fn ungrant(client: &Client, id: &str, service: &str) -> Result<()> {
    runtime::wait(client.revoke_grant(id, service))?;
    eprintln!("key {id} may no longer call {service}");
    Ok(())
}

/// Set key `id`'s daily budget to `usd`.
pub fn budget(client: &Client, id: &str, usd: u32, json: bool) -> Result<()> {
    let body = UpdateKeyRequest {
        budget: Some(i64::from(usd) * 1_000_000),
        scope: None,
    };
    let key = runtime::wait(client.update_key(id, &body))?;
    if json {
        return terminal::json(&key);
    }
    eprintln!("key {id} spends at most ${usd} a UTC day on tool prices");
    Ok(())
}

/// Replace key `id`'s token. The new token is printed once.
pub fn roll(client: &Client, id: &str, yes: bool, json: bool) -> Result<()> {
    terminal::confirm(yes, &format!("roll key {id}; its token stops working"))?;
    let rolled = runtime::wait(client.roll_key(id))?;
    if json {
        return terminal::json(&rolled);
    }
    println!("{}", rolled.token);
    eprintln!("key {id} rolled; the token above is shown once");
    Ok(())
}
