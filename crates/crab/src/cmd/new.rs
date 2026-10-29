//! `crab new` — scaffold a program.

use crate::project;
use anyhow::{Context, Result, bail};
use std::{fs, path::Path};

/// The release a scaffold's `berm-lang` comes from: crab's tag `v<SDK>`, the
/// one this binary was released as.
const SDK: &str = env!("CARGO_PKG_VERSION");

/// Each template beside the path it is written to. Each placeholder is a valid
/// Rust identifier, and each file parses as the language it is.
const FILES: [(&str, &str); 4] = [
    ("Cargo.toml", include_str!("../../templates/manifest.toml")),
    (project::FILE, include_str!("../../templates/crab.toml")),
    ("src/lib.rs", include_str!("../../templates/lib.rs")),
    ("tests/tools.rs", include_str!("../../templates/tools.rs")),
];

pub fn run(name: &str) -> Result<()> {
    check(name)?;

    let root = Path::new(name);
    if root.exists() {
        bail!("{name} already exists");
    }

    let krate = name.replace('-', "_");
    for (path, template) in FILES {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        let contents = template
            .replace("__NAME__", name)
            .replace("__CRATE__", &krate)
            .replace("__VERSION__", SDK);
        fs::write(&path, contents).with_context(|| format!("cannot write {}", path.display()))?;
    }

    println!("created {name}/");
    println!();
    println!("  cd {name}");
    println!("  cargo test");
    println!("  crab build");
    println!("  crab deploy");
    Ok(())
}

/// A program name is a crate name, held to cargo's rules.
fn check(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("a program needs a name");
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        bail!("{name:?} starts with a digit, which a crate name cannot");
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && *c != '-' && *c != '_')
    {
        bail!("{name:?} contains {bad:?}; a crate name takes letters, digits, `-` and `_`");
    }
    Ok(())
}
