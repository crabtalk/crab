//! `crab visibility` — who a service answers.

use crate::{runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::client::Client;

/// Print who `name` answers, or with `private`, set it.
pub fn run(client: &Client, name: &str, private: Option<bool>, json: bool) -> Result<()> {
    if let Some(private) = private {
        runtime::wait(client.set_visibility(name, private))?;
    }
    let visibility = runtime::wait(client.visibility(name))?;
    if json {
        return terminal::json(&visibility);
    }
    match visibility.private {
        true => println!("{name} is private: only its account and members see it"),
        false => println!("{name} is public"),
    }
    Ok(())
}
