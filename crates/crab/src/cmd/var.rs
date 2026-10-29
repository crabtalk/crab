//! `crab var` — a service's vars: listed, set, returned to their defaults.

use crate::{runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn ls(client: &Client, name: &str, json: bool) -> Result<()> {
    let vars = runtime::wait(client.vars(name))?;
    if json {
        return terminal::json(&vars);
    }
    if vars.is_empty() {
        println!("{name} declares no [vars]");
    }
    for var in &vars {
        match &var.value {
            Some(value) => println!("{}  {value}", var.name),
            None => println!("{}  {}  (default)", var.name, var.default),
        }
        if !var.description.is_empty() {
            println!("    {}", var.description);
        }
    }
    Ok(())
}

/// Set `var` to `value`, or return it to its default on `None`.
pub fn set(client: &Client, name: &str, var: &str, value: Option<&str>) -> Result<()> {
    runtime::wait(client.set_var(name, var, value))?;
    match value {
        Some(_) => eprintln!("set {name} {var}, read from its next call"),
        None => eprintln!("{name} {var} reads its default"),
    }
    Ok(())
}
