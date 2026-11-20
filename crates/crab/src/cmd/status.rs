//! `crab status` — where `crab` is talking, and as whom.

use crate::{
    context::{self, Contexts},
    format, runtime, terminal,
};
use anyhow::Result;
use crabtalk_sdk::{
    account::{auth::Status as AuthStatus, key::Scope},
    client::{Client, Error},
};
use serde::Serialize;

#[derive(Serialize)]
struct Status {
    context: String,
    host: String,
    serving: bool,
    /// The account a Cloud context acts for; absent on the local host.
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<String>,
    credential: AuthStatus,
}

/// Status of the context `named`, or of the one in use.
pub fn run(client: &Client, named: Option<&str>, json: bool) -> Result<()> {
    let contexts = Contexts::load()?;
    let context = named
        .or(contexts.current.as_deref())
        .unwrap_or(context::DEFAULT)
        .to_owned();
    let serving = runtime::wait(client.health()).unwrap_or(false);
    let credential = if client.owner.is_some() {
        runtime::wait(client.auth_status())?
    } else {
        AuthStatus::Local { caller: None }
    };
    let account = client.owner.as_ref().map(|owner| owner.name().to_owned());
    let plan = if client.owner.is_some() && credential.allows(Scope::UsageRead) {
        match runtime::wait(client.period()) {
            Ok(period) => Some(period),
            Err(Error::Api { status: 403, .. }) => None,
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
    let status = Status {
        context,
        host: client.base.clone(),
        serving,
        account,
        plan: plan.as_ref().map(|period| period.plan.clone()),
        credential,
    };
    if json {
        return terminal::json(&status);
    }
    println!("context  {}", status.context);
    println!(
        "host     {}{}",
        status.host,
        if serving { "" } else { "  (not answering)" }
    );
    if let Some(account) = &status.account {
        println!("account  {account}");
    }
    match (&status.account, plan) {
        (Some(_), Some(period)) => {
            println!(
                "plan     {}, {} through {}",
                period.plan,
                format::day(period.since.into()),
                format::day(i64::from(period.until) - 1)
            );
        }
        (None, _) => println!("the local host: no account"),
        _ => {}
    }
    match &status.credential {
        AuthStatus::ApiKey { key } => {
            println!("key      {} ({})", key.id, key.label);
            println!("key account  {}", key.owner_id);
            println!(
                "permissions  {}",
                key.scope
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "budget   {} of {} today, UTC",
                format::usd(key.spent as f64),
                format::usd(key.budget as f64)
            );
            println!(
                "expires  {}",
                key.expires_at
                    .map(format::date)
                    .unwrap_or_else(|| "never".to_owned())
            );
            for service in &key.services {
                println!("may call {}", service.service);
            }
        }
        AuthStatus::Session {
            account,
            client,
            expires_at,
        } => {
            println!("session  {client}, account {account}");
            println!("expires  {}", format::date(*expires_at));
        }
        AuthStatus::Local { .. } => {}
    }
    Ok(())
}
