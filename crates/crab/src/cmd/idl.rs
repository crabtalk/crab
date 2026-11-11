//! `crab idl` — a deployed service's IDL, for `declare_service!`.

use crate::runtime;
use anyhow::{Context, Result};
use crabtalk_sdk::client::Client;
use std::{fs, path::Path};

/// Write `name`'s IDL to `{dir}/{name}.json`.
pub fn run(client: &Client, name: &str, dir: &Path) -> Result<()> {
    let service = runtime::wait(client.inspect(name))?;
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(format!("{}.json", service.name));
    let mut idl = serde_json::to_value(&service)?;
    if let Some(owner) = name
        .split_once('/')
        .map(|(owner, _)| owner)
        .or_else(|| client.owner.as_ref().map(|owner| owner.name()))
    {
        idl["owner"] = owner.into();
    }
    let json = serde_json::to_string_pretty(&idl)?;
    fs::write(&path, json + "\n").with_context(|| format!("writing {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}
