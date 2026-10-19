//! `crab rm` — take a program out of service.

use crate::utils;
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(client: &Client, name: &str, yes: bool) -> Result<()> {
    utils::confirm(yes, &format!("remove {name} and what it committed"))?;
    utils::wait(client.undeploy(name))?;
    eprintln!("removed {name}");
    Ok(())
}
