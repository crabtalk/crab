//! `crab request` — what the account asked a person to do, by its code: where
//! it stands, or once it is no longer pending.

use crate::{format, utils};
use anyhow::{Result, bail};
use crabtalk_sdk::{
    account::request::{Purchase, Request, RequestKind, RequestStatus},
    client::Client,
};
use serde::Serialize;
use std::{
    thread,
    time::{Duration, Instant},
};

/// How often `--wait` asks after a request.
const POLL: Duration = Duration::from_secs(2);

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
    if request.kind == RequestKind::Secret {
        let names: Vec<&str> = request.secrets.iter().map(|s| s.name.as_str()).collect();
        return format!(
            "set {} of {}, signed in",
            names.join(", "),
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
        Some(Purchase::Trial) => format!(
            "a {plan} trial, free for {} days, then {monthly}",
            request.trial_days.unwrap_or_default()
        ),
        Some(Purchase::Credit) => "credit, the amount chosen at checkout".to_owned(),
        None => String::new(),
    }
}

/// Where request `code` stands. With `wait`, once it is no longer pending,
/// giving up after `timeout` seconds. Fails unless it completed or, without
/// `wait`, is still pending.
pub fn run(client: &Client, code: &str, wait: bool, timeout: u64, json: bool) -> Result<()> {
    let started = Instant::now();
    let request = loop {
        let request = utils::wait(client.request(code))?;
        let waiting = request.status == RequestStatus::Pending
            && started.elapsed() < Duration::from_secs(timeout);
        if !wait || !waiting {
            break request;
        }
        thread::sleep(POLL);
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
