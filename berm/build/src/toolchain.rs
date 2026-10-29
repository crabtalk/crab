//! The Rust target a service builds for, added to the toolchain on first use.

use crate::TARGET;
use anyhow::{Context, Result, bail};
use std::{env, path::Path, process::Command};

/// Add [`TARGET`] to the toolchain `dir` builds with, through rustup, when its
/// sysroot lacks it. `dir` decides the toolchain, so a `rust-toolchain.toml`
/// there is the one checked and changed.
pub fn ensure(dir: &Path) -> Result<()> {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let out = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .current_dir(dir)
        .output()
        .with_context(|| format!("cannot run {rustc}"))?;
    if !out.status.success() {
        bail!("{rustc} --print sysroot failed");
    }
    let sysroot =
        String::from_utf8(out.stdout).context("rustc printed a sysroot that is not UTF-8")?;
    if Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(TARGET)
        .is_dir()
    {
        return Ok(());
    }

    let added = Command::new("rustup")
        .args(["target", "add", TARGET])
        .current_dir(dir)
        .status();
    match added {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => bail!("rustup could not add {TARGET}"),
        Err(_) => bail!("this toolchain has no {TARGET} and rustup is not installed to add it"),
    }
}
