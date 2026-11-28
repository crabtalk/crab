//! `crab domain` — a service's custom domains: listed, added, removed.

use crate::{cmd::request, runtime, terminal};
use anyhow::{Context, Result};
use crabtalk_sdk::{
    client::Client,
    services::domain::{Domain, DomainStatus},
};

pub fn ls(client: &Client, name: &str, json: bool) -> Result<()> {
    let domains = runtime::wait(client.domains(name))?;
    if json {
        return terminal::json(&domains);
    }
    if domains.is_empty() {
        println!("{name} has no custom domains");
    }
    for domain in &domains {
        show(domain);
    }
    Ok(())
}

pub fn add(client: &Client, name: &str, host: &str, json: bool) -> Result<()> {
    let domain = match request::approved(client, runtime::wait(client.add_domain(name, host)))? {
        Some(domain) => domain,
        None => runtime::wait(client.domains(name))?
            .into_iter()
            .find(|domain| domain.host.eq_ignore_ascii_case(host.trim_end_matches('.')))
            .context("the approved domain is not listed")?,
    };
    if json {
        return terminal::json(&domain);
    }
    show(&domain);
    eprintln!(
        "create these records at {host}'s DNS provider; `crab domain ls {name}` shows when it is active"
    );
    Ok(())
}

pub fn rm(client: &Client, name: &str, host: &str) -> Result<()> {
    runtime::wait(client.remove_domain(name, host))?;
    eprintln!("{name} is no longer served at {host}");
    Ok(())
}

fn show(domain: &Domain) {
    let status = match domain.status {
        DomainStatus::Pending => "pending",
        DomainStatus::Active => "active",
        DomainStatus::Failed => "failed",
    };
    println!("{}  {status}", domain.host);
    if domain.status != DomainStatus::Active {
        for record in &domain.records {
            println!("    {}  {}  {}", record.kind, record.name, record.value);
        }
    }
    for error in &domain.errors {
        println!("    {error}");
    }
}
