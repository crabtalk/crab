//! `crab build` — build a service's image.

use crate::project;
use anyhow::Result;
use std::path::Path;

/// Build the service in `dir` and print the path of its finished image.
pub fn run(dir: &Path) -> Result<()> {
    project::read(dir)?;
    println!("{}", berm_build::build(dir, None)?.display());
    Ok(())
}
