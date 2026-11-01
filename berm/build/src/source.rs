use anyhow::{Context, Result, ensure};
use std::{fs, path::Path, process::Command};

/// Cache by source URL; publish a new download only after curl succeeds.
pub fn download(url: &str, destination: &Path) -> Result<()> {
    let source = destination.with_extension("source");
    if destination.exists() && fs::read_to_string(&source).is_ok_and(|cached| cached == url) {
        return Ok(());
    }
    let temporary = destination.with_extension("download");
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--retry",
            "2",
            "--max-time",
            "60",
            "--output",
        ])
        .arg(&temporary)
        .arg(url)
        .status()
        .context("running curl to download build source")?;
    ensure!(status.success(), "build source download failed: {url}");
    fs::rename(&temporary, destination).context("saving downloaded build source")?;
    fs::write(source, url).context("saving build source URL")
}
