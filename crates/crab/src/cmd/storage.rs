use crate::{cmd::export, format, runtime, terminal};
use anyhow::Result;
use clap::Subcommand;
use crabtalk_sdk::{client::Client, format::bytes};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Action {
    /// List storage and plan usage.
    List,
    /// Download one caller's SQLite file.
    Download {
        id: String,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Delete one file from caller storage.
    Delete { id: String },
}

pub fn run(client: &Client, action: Option<&Action>, json: bool, yes: bool) -> Result<()> {
    match action {
        None | Some(Action::List) => {
            let usage = runtime::wait(client.caller_storage())?;
            if json {
                return terminal::json(&usage);
            }
            println!(
                "{} of {} used on this plan",
                bytes(usage.used),
                bytes(usage.limit)
            );
            if !usage.storages.is_empty() {
                println!("{:<36}  {:>10}  {:<20}  SERVICE", "ID", "SIZE", "LAST CALL");
            }
            for entry in usage.storages {
                let status = if entry.deleted { " (deleted)" } else { "" };
                println!(
                    "{:<36}  {:>10}  {:<20}  {}/{}{}",
                    entry.id,
                    bytes(entry.size),
                    format::instant(entry.called_at as u64 * 1000),
                    entry.owner,
                    entry.name,
                    status
                );
            }
        }
        Some(Action::Download { id, out }) => {
            let id = uuid::Uuid::parse_str(id)?.to_string();
            let default = PathBuf::from(format!("{id}.sqlite"));
            export::save(
                client.download_caller_storage(&id),
                out.as_deref().unwrap_or(&default),
            )?;
        }
        Some(Action::Delete { id }) => {
            let id = uuid::Uuid::parse_str(id)?.to_string();
            terminal::confirm(yes, &format!("Delete caller storage {id}"))?;
            runtime::wait(client.delete_caller_storage(&id))?;
        }
    }
    Ok(())
}

pub fn callers(client: &Client, service: &str, json: bool) -> Result<()> {
    let callers = runtime::wait(client.stored_callers(service))?;
    if json {
        return terminal::json(&callers);
    }
    if callers.is_empty() {
        println!("No callers have written to storage yet.");
        return Ok(());
    }
    println!(
        "{:>10}  {:<20}  {:>8}  CALLER",
        "SIZE", "LAST CALL", "SCHEMA"
    );
    for caller in callers {
        let end_user = caller.issuer.as_deref().zip(caller.subject.as_deref());
        let identity = match end_user {
            Some((issuer, subject)) => format!("{} {subject} ({issuer})", caller.account),
            None => caller.account,
        };
        println!(
            "{:>10}  {:<20}  {:>8}  {identity}",
            bytes(caller.size),
            format::instant(caller.called_at as u64 * 1000),
            caller.schema
        );
    }
    Ok(())
}
