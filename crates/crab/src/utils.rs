//! Running the SDK's async calls from `crab`'s synchronous commands, and what
//! every command shares about talking to the person at the terminal.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    env,
    future::Future,
    io::{BufRead, IsTerminal, Write},
    path::PathBuf,
    process::Command,
    sync::LazyLock,
};
use tokio::runtime::{Builder, Runtime};

/// One runtime for the process, on every core for the local host's calls.
/// `crab` stays synchronous because `berm-oci` uses blocking `reqwest`, which
/// panics inside a runtime.
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime builds")
});

/// `$HOME`.
pub fn home() -> Result<PathBuf> {
    Ok(PathBuf::from(env::var("HOME").context("HOME is not set")?))
}

/// Run `future` to completion.
pub fn wait<F: Future>(future: F) -> F::Output {
    RUNTIME.block_on(future)
}

/// Print `value` as one line of JSON.
pub fn json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

/// Go on with `what` only when `yes` is set or the person at the terminal
/// says so. Without a terminal it refuses rather than waits.
pub fn confirm(yes: bool, what: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        bail!("{what} needs `--yes` without a terminal");
    }
    eprint!("{what}? [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    match answer.trim() {
        "y" | "Y" | "yes" => Ok(()),
        _ => bail!("cancelled"),
    }
}

/// Print `url` and open it in the browser, answering whether one opened.
pub fn open(url: &str) -> bool {
    eprintln!("{url}");
    #[cfg(target_os = "macos")]
    let opened = Command::new("open").arg(url).status();
    #[cfg(target_os = "windows")]
    let opened = Command::new("cmd").args(["/C", "start", "", url]).status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let opened = Command::new("xdg-open").arg(url).status();
    opened.is_ok_and(|status| status.success())
}
