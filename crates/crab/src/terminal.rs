//! What every command shares about talking to the person at the terminal.

use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    io::{self, BufRead, IsTerminal, Write},
    process::Command,
};

/// Print `value` as one line of JSON.
pub fn json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

/// Go on with `what` only when `yes` is set or the person at the terminal
/// says so. Without a terminal it refuses.
pub fn confirm(yes: bool, what: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        bail!("{what} needs `--yes` without a terminal");
    }
    eprint!("{what}? [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
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
