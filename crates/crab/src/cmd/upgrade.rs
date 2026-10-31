//! `crab upgrade` — replace this `crab` with the latest release.

use crate::terminal;
use anyhow::Result;
use self_update::{backends::github::Update, cargo_crate_version};
use std::io::{self, IsTerminal};

/// Upgrade to the newest release when it is newer than this `crab`.
pub fn run(yes: bool) -> Result<()> {
    let current = cargo_crate_version!();
    let mut updater = Update::configure();
    updater
        .repo_owner("crabtalk")
        .repo_name("crab")
        .bin_name("crab")
        .current_version(current)
        .show_output(false)
        .show_download_progress(io::stderr().is_terminal())
        .no_confirm(true);
    let newer = updater.build()?.get_newer_releases()?;
    let Some(latest) = newer.latest() else {
        eprintln!("no release is newer than crab {current}");
        return Ok(());
    };
    let version = latest.version().to_owned();
    terminal::confirm(yes, &format!("upgrade crab {current} to {version}"))?;
    // Pinned to the release just confirmed, not whichever is latest by now.
    updater
        .release_tag(format!("v{version}"))
        .build()?
        .update()?;
    eprintln!("upgraded crab to {version}");
    Ok(())
}
