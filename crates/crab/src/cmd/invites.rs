//! `crab invites` — the orgs that invited you, and joining one.

use crate::{format, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn ls(client: &Client, json: bool) -> Result<()> {
    let invites = runtime::wait(client.my_invites())?;
    if json {
        return terminal::json(&invites);
    }
    if invites.is_empty() {
        println!("no invitations");
    }
    for invite in &invites {
        println!(
            "{:<24}  {:<8}  expires {}  {}",
            invite.org_name,
            invite.role,
            format::date(invite.expires_at),
            invite.token
        );
    }
    Ok(())
}

pub fn accept(client: &Client, token: &str) -> Result<()> {
    runtime::wait(client.accept_invite(token))?;
    eprintln!("joined");
    Ok(())
}
