//! `crab upgrade` — replace this `crab` with the latest release.

use crate::utils;
use anyhow::Result;
use self_update::{
    backends::github::{Update, UpdateBuilder},
    cargo_crate_version,
};
use std::io::IsTerminal;

/// An updater for `crabtalk/crab`'s releases, replacing the running `crab`.
fn updater() -> UpdateBuilder {
    let mut builder = Update::configure();
    builder
        .repo_owner("crabtalk")
        .repo_name("crab")
        .bin_name("crab")
        .current_version(cargo_crate_version!())
        .show_output(false)
        .show_download_progress(std::io::stderr().is_terminal())
        .no_confirm(true);
    builder
}

/// Upgrade to the newest release when it is newer than this `crab`.
pub fn run(yes: bool) -> Result<()> {
    let current = cargo_crate_version!();
    let mut updater = updater();
    let newer = updater.build()?.get_newer_releases()?;
    let Some(latest) = newer.latest() else {
        eprintln!("no release is newer than crab {current}");
        return Ok(());
    };
    let version = latest.version().to_owned();
    utils::confirm(yes, &format!("upgrade crab {current} to {version}"))?;
    // Pinned to the release just confirmed, not whichever is latest by now.
    updater
        .release_tag(format!("v{version}"))
        .build()?
        .update()?;
    eprintln!("upgraded crab to {version}");
    Ok(())
}
