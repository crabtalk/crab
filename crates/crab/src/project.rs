//! A program's or kit's `crab.toml`: reading it, and writing it with its
//! layout kept as written.

use anyhow::{Context, Result};
use berm_api::project::Project;
use std::{collections::BTreeMap, fs, path::Path};

/// What the project file beside a program's or kit's `Cargo.toml` is called.
pub const FILE: &str = "crab.toml";

/// The `crab.toml` in `dir`.
pub fn read(dir: &Path) -> Result<Project> {
    Project::read(&dir.join(FILE))
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
