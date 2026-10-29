//! `crab idl` — a deployed service's IDL, for `declare_service!`.

use crate::runtime;
use anyhow::{Context, Result};
use crabtalk_sdk::client::Client;
use std::{fs, path::Path};

/// Write `name`'s IDL to `{dir}/{name}.json`.
pub fn run(client: &Client, name: &str, dir: &Path) -> Result<()> {
    let service = runtime::wait(client.inspect(name))?;
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(format!("{name}.json"));
    let json = serde_json::to_string_pretty(&service)?;
    fs::write(&path, json + "\n").with_context(|| format!("writing {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}
