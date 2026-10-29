//! `crab explain` — what an error code means and what to do about it, as
//! `rustc --explain` answers for the compiler's.

use crate::{runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(client: &Client, code: &str, json: bool) -> Result<()> {
    let explanation = runtime::wait(client.explain(&code.to_ascii_uppercase()))?;
    if json {
        return terminal::json(&explanation);
    }
    println!("{}", explanation.text.trim_end());
    Ok(())
}
