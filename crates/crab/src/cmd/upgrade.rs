//! `crab upgrade` — replace this `crab` with the latest release.

use crate::utils;
use anyhow::Result;
use self_update::{
    backends::github::{Update, UpdateBuilder},
    cargo_crate_version, get_target,
};
use std::io::IsTerminal;

/// The release asset built for the target this `crab` was built for. The
/// release workflow uploads it, with `{asset}.sha256` beside it, and the
/// install script on crab.talk downloads it.
fn asset() -> String {
    let archive = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!("crab-{}.{archive}", get_target())
}

/// An updater for `crabtalk/crab`'s releases, replacing the running `crab`.
fn updater() -> UpdateBuilder {
    let asset = asset();
    let mut builder = Update::configure();
    builder
        .repo_owner("crabtalk")
        .repo_name("crab")
        .bin_name("crab")
        .current_version(cargo_crate_version!())
        .checksum_from_asset(format!("{asset}.sha256"))
        // Exact: the default match is by substring, which the `.sha256`
        // beside the archive also passes.
        .asset_matcher(move |assets| assets.iter().find(|a| a.name() == asset).cloned())
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
