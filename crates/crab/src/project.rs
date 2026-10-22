//! Writing a program's `berm.toml`, its layout kept as written.

use anyhow::{Context, Result};
use berm_api::project::Project;
use std::{collections::BTreeMap, fs, path::Path};
use toml_edit::{DocumentMut, Item, Table, value};

/// Write `crons` as `[crons]` of the `berm.toml` in `dir`, in place of the one
/// there; the rest of the file is kept as written.
pub fn write_crons(dir: &Path, crons: &BTreeMap<String, String>) -> Result<()> {
    let path = dir.join(Project::FILE);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("no {} in {}", Project::FILE, dir.display()))?;
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
