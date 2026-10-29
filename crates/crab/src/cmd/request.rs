//! `crab request` — what the account asked a person to do, by its code: where
//! it stands, or once it is no longer pending.

use crate::{format, runtime, terminal};
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
        return terminal::json(&Asked {
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
    if request.kind == RequestKind::Verify {
        return "verify on GitHub, signed in".to_owned();
    }
    if request.kind == RequestKind::Deploy {
        let services: Vec<&str> = request.deploys.keys().map(String::as_str).collect();
        return format!("approve deploying {}, signed in", services.join(", "));
    }
    if let Some(access) = &request.access {
        let mut asked: Vec<String> = access
            .services
            .iter()
            .map(|granted| format!("call {}", granted.service))
            .collect();
        if let Some(budget) = access.budget {
            asked.push(format!("spend {} a day", format::usd(budget as f64)));
        }
        return format!(
            "let key {} {}, signed in",
            access.label,
            asked.join(" and ")
        );
    }
    if request.kind == RequestKind::Grant {
        return "let a key spend, signed in".to_owned();
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
        Some(Purchase::Plan) => format!("{plan} at {monthly}"),
        Some(Purchase::Credit) => "credit, the amount chosen at checkout".to_owned(),
        None => String::new(),
    }
}

/// End request `code` now, and print it.
pub fn expire(client: &Client, code: &str, json: bool) -> Result<()> {
    let request = runtime::wait(client.expire_request(code))?;
    if json {
        return terminal::json(&request);
    }
    println!(
        "{}  {}  {}",
        request.code,
        request.status,
        describe(&request)
    );
    Ok(())
}

/// Request `code` once it is no longer pending, or as it stands after
/// `timeout` seconds.
pub fn settled(client: &Client, code: &str, timeout: u64) -> Result<Request> {
    let until = Instant::now() + Duration::from_secs(timeout);
    loop {
        let left = until.saturating_duration_since(Instant::now()).as_secs();
        let request = runtime::wait(client.request(code, left.min(HOLD)))?;
        if request.status != RequestStatus::Pending || left == 0 {
            return Ok(request);
        }
    }
}

/// Where request `code` stands. With `wait`, once it is no longer pending,
/// giving up after `timeout` seconds. Fails unless it completed or, without
/// `wait`, is still pending.
pub fn run(client: &Client, code: &str, wait: bool, timeout: u64, json: bool) -> Result<()> {
    let request = settled(client, code, if wait { timeout } else { 0 })?;
    if json {
        terminal::json(&request)?;
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
