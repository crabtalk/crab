//! `crab request` — what the account asked a person to do, by its code: where
//! it stands, or once it is no longer pending.

use crate::{format, utils};
use anyhow::{Result, bail};
use crabtalk_sdk::{
    account::request::{Purchase, Request, RequestKind, RequestStatus},
    client::Client,
};
use serde::Serialize;
use std::time::{Duration, Instant};

/// Seconds Cloud holds each ask of `--wait` while the request is pending.
const HOLD: u64 = 60;

/// Print a request just made: its page, what it asks, and how to wait for it.
pub fn asked(request: &Request, json: bool) -> Result<()> {
    let wait = format!("crab request {} --wait", request.code);
    if json {
        #[derive(Serialize)]
        struct Next<'a> {
            command: &'a str,
            until: &'a str,
        }
        #[derive(Serialize)]
        struct Asked<'a> {
            #[serde(flatten)]
            request: &'a Request,
            next: Next<'a>,
        }
        return utils::json(&Asked {
            request,
            next: Next {
                command: &wait,
                until: "status is no longer pending",
            },
        });
    }
    let section = request
        .url
        .trim_end_matches(&request.code)
        .trim_end_matches('/');
    println!("{}", request.url);
    eprintln!(
        "{}: open the link, or enter {} at {section}, within {} minutes",
        describe(request),
        request.code,
        (request.expires_at - request.created_at) / 60
    );
    eprintln!("then `{wait}`");
    Ok(())
}

/// What `request` asks, priced when it is a payment.
fn describe(request: &Request) -> String {
    if request.kind == RequestKind::Login {
        return "sign crab in".to_owned();
    }
    if request.kind == RequestKind::Secret {
        let asked: Vec<&str> = request
            .credentials
            .iter()
            .map(|credential| match credential.title.is_empty() {
                true => credential.id.as_str(),
                false => credential.title.as_str(),
            })
            .collect();
        return format!(
            "set {} of {}, signed in",
            asked.join(", "),
            request.service.as_deref().unwrap_or_default()
        );
    }
    let plan = request.plan.as_deref().unwrap_or_default();
    let monthly = request
        .price_micros
        .map(|micros| format!("{} a month", format::usd(micros as f64)))
        .unwrap_or_default();
    match request.purchase {
        Some(Purchase::Plan) if request.card_on_file => {
            format!("{plan} at {monthly}, ending the trial and charging its card at once")
        }
        Some(Purchase::Plan) => format!("{plan} at {monthly}"),
        Some(Purchase::Trial) => match &request.trial {
            Some(trial) if !trial.card => format!(
                "a {plan} trial, free for {} days, then {monthly}, no card needed",
                trial.days
            ),
            Some(trial) => format!(
                "a {plan} trial, free for {} days, then {monthly}, a card needed: {}",
                trial.days,
                trial.reason.as_deref().unwrap_or_default()
            ),
            None => format!("a {plan} trial, then {monthly}"),
        },
        Some(Purchase::Credit) => "credit, the amount chosen at checkout".to_owned(),
        None => String::new(),
    }
}

/// End request `code` now, and print it.
pub fn expire(client: &Client, code: &str, json: bool) -> Result<()> {
    let request = utils::wait(client.expire_request(code))?;
    if json {
        return utils::json(&request);
    }
    println!(
        "{}  {}  {}",
        request.code,
        request.status,
        describe(&request)
    );
    Ok(())
}

/// Where request `code` stands. With `wait`, once it is no longer pending,
/// giving up after `timeout` seconds. Fails unless it completed or, without
/// `wait`, is still pending.
pub fn run(client: &Client, code: &str, wait: bool, timeout: u64, json: bool) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(timeout);
    let request = loop {
        let left = until.saturating_duration_since(Instant::now()).as_secs();
        let hold = if wait { left.min(HOLD) } else { 0 };
        let request = utils::wait(client.request(code, hold))?;
        if request.status != RequestStatus::Pending || hold == 0 {
            break request;
        }
    };
    if json {
        utils::json(&request)?;
    } else {
        println!(
            "{}  {}  {}",
            request.code,
            request.status,
            describe(&request)
        );
    }
    match request.status {
        RequestStatus::Completed => Ok(()),
        RequestStatus::Pending if wait => bail!("still pending after {timeout}s"),
        RequestStatus::Pending => Ok(()),
        RequestStatus::Declined => bail!("the person declined it"),
        RequestStatus::Expired => bail!("it expired; ask again for a new code"),
    }
}
