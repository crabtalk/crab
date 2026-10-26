//! `crab ls` — what is deployed.

use crate::utils;
use anyhow::Result;
use berm_api::Program;
use crabtalk_sdk::client::Client;
use serde::Serialize;

/// Enough of a digest to tell two images apart at a glance, which is all a
/// listing is for. `inspect` prints the whole one.
const SHORT: usize = 12;

/// A deployed program, and whether it is stopped.
#[derive(Serialize)]
struct Listed {
    #[serde(flatten)]
    program: Program,
    stopped: bool,
}

pub fn run(client: &Client, json: bool) -> Result<()> {
    let mut programs = Vec::new();
    for program in utils::wait(client.list())? {
        let stopped = utils::wait(client.runtime(&program.name))?.stopped;
        programs.push(Listed { program, stopped });
    }
    if json {
        return utils::json(&programs);
    }
    if programs.is_empty() {
        println!("no programs deployed");
        return Ok(());
    }

    let width = programs
        .iter()
        .map(|listed| listed.program.name.len())
        .max()
        .unwrap_or(0)
        .max("NAME".len());

    println!(
        "{:<width$}  {:<SHORT$}  {:<7}  TOOLS",
        "NAME", "DIGEST", "STATE"
    );
    for Listed { program, stopped } in &programs {
        let state = match stopped {
            true => "stopped",
            false => "running",
        };
        println!(
            "{:<width$}  {:<SHORT$}  {state:<7}  {}",
            program.name,
            digest(program),
            tools(program),
        );
    }
    Ok(())
}

fn digest(program: &Program) -> &str {
    program.digest.get(..SHORT).unwrap_or(&program.digest)
}

fn tools(program: &Program) -> String {
    program
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
