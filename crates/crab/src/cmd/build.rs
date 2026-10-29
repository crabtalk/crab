//! `crab build` — build a program's image.

use crate::project;
use anyhow::Result;
use std::path::Path;

/// Build the program in `dir` and print the path of its finished image.
pub fn run(dir: &Path) -> Result<()> {
    project::read(dir)?;
    println!("{}", berm_build::build(dir, None)?.display());
    Ok(())
}
