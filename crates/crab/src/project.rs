//! A program's or kit's `crab.toml`: reading it, and writing it with its
//! layout kept as written.

use anyhow::{Context, Result, bail};
use berm_api::project::Project;
use berm_build::Workspace;
use std::{collections::BTreeMap, fs, path::Path};

/// What the project file beside a program's or kit's `Cargo.toml` is called.
pub const FILE: &str = "crab.toml";

/// The `crab.toml` in `dir`.
pub fn read(dir: &Path) -> Result<Project> {
    Project::read(&dir.join(FILE))
}

/// The services `names` names, or with none, those the workspace the current
/// directory is in selects as `crab deploy` does. Fails when that is none.
pub fn selected(names: &[String]) -> Result<Vec<String>> {
    if !names.is_empty() {
        return Ok(names.to_vec());
    }
    let workspace = Workspace::read(Path::new("."), FILE, &[])?;
    let mut names = Vec::new();
    for dir in &workspace.services {
        names.push(read(dir)?.name()?.to_owned());
    }
    if names.is_empty() {
        bail!("no service here; name one");
    }
    Ok(names)
}
use toml_edit::{DocumentMut, Item, Table, value};

/// Write `crons` as `[crons]` of the `crab.toml` in `dir`, in place of the one
/// there; the rest of the file is kept as written.
pub fn write_crons(dir: &Path, crons: &BTreeMap<String, String>) -> Result<()> {
    let path = dir.join(FILE);
    let text = fs::read_to_string(&path).with_context(|| format!("no {}", path.display()))?;
    let mut file: DocumentMut = text
        .parse()
        .with_context(|| format!("cannot parse {}", path.display()))?;
    let mut table = Table::new();
    for (tool, cron) in crons {
        table.insert(tool, value(cron));
    }
    file.insert("crons", Item::Table(table));
    fs::write(&path, file.to_string()).with_context(|| format!("cannot write {}", path.display()))
}
