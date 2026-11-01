use anyhow::{Context, Result};
use berm_build::fixture;
use std::{env, fs::OpenOptions, io::Write};

fn main() -> Result<()> {
    let package = env::args()
        .nth(1)
        .context("specify the Cargo package to prepare")?;
    let output =
        env::var_os("NEXTEST_ENV").context("run this example as a nextest setup script")?;
    let image = berm_build::build(&env::current_dir()?, Some(&package))?;
    let mut output = OpenOptions::new().append(true).open(output)?;
    writeln!(
        output,
        "{}={}",
        fixture::variable(&package),
        image.display()
    )?;
    Ok(())
}
