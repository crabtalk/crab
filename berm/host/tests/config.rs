//! A host's config read from TOML: durations as humans write them.

use anyhow::Result;
use berm_host::Config;
use std::time::Duration;

#[test]
fn durations_read_as_written() -> Result<()> {
    let config: Config = toml::from_str(
        "cpu = \"50ms\"\ndeadline = \"60s\"\ncode_idle = \"2m\"\ncode_cache = 1048576\n",
    )?;
    assert_eq!(config.cpu, Duration::from_millis(50));
    assert_eq!(config.deadline, Duration::from_secs(60));
    assert_eq!(config.code_idle, Duration::from_secs(120));
    assert_eq!(config.code_cache, 1 << 20);
    Ok(())
}

#[test]
fn a_missing_duration_takes_its_default() -> Result<()> {
    let config: Config = toml::from_str("")?;
    assert_eq!(config.cpu, Config::default().cpu);
    assert_eq!(config.deadline, Config::default().deadline);
    assert_eq!(config.code_idle, Config::default().code_idle);
    assert_eq!(config.code_cache, Config::default().code_cache);
    Ok(())
}
