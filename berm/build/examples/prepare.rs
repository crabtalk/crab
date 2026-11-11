use anyhow::{Context, Result};
use berm_build::fixture;
use std::{
    env,
    fs::OpenOptions,
    io::{BufWriter, Write},
};

const PACKAGES: &[&str] = &[
    "berm-fixture",
    "berm-bare",
    "berm-broken",
    "berm-feed",
    "berm-account",
    "berm-billing",
    "berm-keeper",
    "berm-site",
    "crab-agent-memory",
    "crab-agent-relay",
    "crab-uptime",
    "crab-webhook-inbox",
    "crab-analytics",
    "berm-openapi",
    "crab-resend",
    "crab-github",
    "crab-upstash",
    "crab-posthog",
    "crab-osv",
    "berm-linked-services",
];

fn main() -> Result<()> {
    let output =
        env::var_os("NEXTEST_ENV").context("run this example as a nextest setup script")?;
    let mut output = BufWriter::new(OpenOptions::new().append(true).open(output)?);
    let workspace = env::current_dir()?;
    // Separate builds preserve each service's dependency features.
    for package in PACKAGES {
        let image = berm_build::build(&workspace, Some(package))
            .with_context(|| format!("preparing {package}"))?;
        writeln!(output, "{}={}", fixture::variable(package), image.display())?;
    }
    output.flush()?;
    Ok(())
}
