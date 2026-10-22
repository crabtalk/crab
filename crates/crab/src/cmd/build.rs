//! `crab build` — build a program's image.

use anyhow::Result;
use std::path::Path;

/// Build the program in `dir` and print the path of its finished image.
pub fn run(dir: &Path) -> Result<()> {
    println!("{}", berm_build::build(dir, None)?.display());
    Ok(())
}
