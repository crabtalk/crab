//! `crab replay` — run a call that failed again, on another host.

use crate::{cmd::call, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;
use std::process::ExitCode;

/// Run `program`'s call `id`, as `from` recorded it, on `on`: its tool, its
/// arguments and its caller, against `on`'s own state.
pub fn run(from: &Client, on: &Client, program: &str, id: u64) -> Result<ExitCode> {
    let input = utils::wait(from.call_input(program, id))?;
    let running = utils::wait(on.inspect(program))?.digest;
    if input.digest.strip_prefix("sha256:") != Some(running.trim_start_matches("sha256:")) {
        eprintln!(
            "the call ran {}; {program} here runs sha256:{}",
            input.digest,
            running.trim_start_matches("sha256:")
        );
    }
    call::run(
        on,
        program,
        &input.tool,
        Some(&input.args),
        input.caller.as_ref(),
    )
}
