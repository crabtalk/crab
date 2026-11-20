//! `crab build` — build a service's image.

use crate::{format, project};
use anyhow::{Context, Result};
use berm_api::Manifest;
use std::{fs, path::Path};

/// Build the service in `dir`, print the path of its finished image, and each
/// priced workflow step on stderr.
pub fn run(dir: &Path) -> Result<()> {
    project::read(dir)?;
    let image = berm_build::build(dir, None)?;
    let bytes = fs::read(&image).context("cannot read the built service image")?;
    let manifest = Manifest::try_from(bytes.as_slice()).context("invalid service image")?;
    for line in format::step_prices(&manifest) {
        eprintln!("{line}");
    }
    println!("{}", image.display());
    Ok(())
}
