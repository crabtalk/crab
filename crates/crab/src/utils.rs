//! Running the SDK's async calls from `crab`'s synchronous commands, and what
//! every command shares about talking to the person at the terminal.

use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    future::Future,
    io::{BufRead, IsTerminal, Write},
    process::Command,
    sync::LazyLock,
};
use tokio::runtime::{Builder, Runtime};

/// One runtime for the process. `crab` stays synchronous because `berm-oci`
/// uses blocking `reqwest`, which panics inside a runtime.
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime builds")
});

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

/// Print `url` and open it in the browser. A browser that does not open
/// leaves the printed URL.
pub fn open(url: &str) {
    eprintln!("{url}");
    #[cfg(target_os = "macos")]
    let opened = Command::new("open").arg(url).status();
    #[cfg(target_os = "windows")]
    let opened = Command::new("cmd").args(["/C", "start", "", url]).status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let opened = Command::new("xdg-open").arg(url).status();
    if !opened.is_ok_and(|status| status.success()) {
        eprintln!("open the URL above in a browser");
    }
}
