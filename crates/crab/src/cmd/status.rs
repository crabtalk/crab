//! `crab status` — where `crab` is talking, and as whom.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;
use serde::Serialize;

#[derive(Serialize)]
struct Status {
    context: String,
    host: String,
    serving: bool,
    /// The account a Cloud context acts for; absent on a bermd.
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<String>,
}

pub fn run(client: &Client, context: &str, json: bool) -> Result<()> {
    let serving = utils::wait(client.health()).unwrap_or(false);
    let (account, plan) = match &client.owner {
        Some(owner) => {
            let period = utils::wait(client.period())?;
            (Some(owner.name().to_owned()), Some(period))
        }
        None => (None, None),
    };
    let status = Status {
        context: context.to_owned(),
        host: client.base.clone(),
        serving,
        account,
        plan: plan.as_ref().map(|period| period.plan.clone()),
    };
    if json {
        return utils::json(&status);
    }
    println!("context  {}", status.context);
    println!(
        "host     {}{}",
        status.host,
        if serving { "" } else { "  (not answering)" }
    );
    match (&status.account, plan) {
        (Some(account), Some(period)) => {
            println!("account  {account}");
            println!(
                "plan     {}, {} through {}",
                period.plan,
                format::day(period.since.into()),
                format::day(i64::from(period.until) - 1)
            );
        }
        _ => println!("a bermd: no account"),
    }
    Ok(())
}
