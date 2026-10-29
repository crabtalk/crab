//! `crab call` — call one of a service's tools.

use crate::runtime;
use anyhow::{Context, Result};
use berm_api::{
    Output,
    host::{Caller, User},
};
use clap::Args;
use crabtalk_sdk::client::Client;
use std::{
    io::{IsTerminal, Read},
    process::ExitCode,
};

/// A program that ran and said no, which is not berm failing to run it — the
/// two leave by different doors so a script can tell them apart.
const FAILED: u8 = 2;

/// What `crab call` is given.
#[derive(Args)]
pub struct Call {
    /// The service: a name on the context's account, or `owner/name` on
    /// Cloud for another account's public service.
    pub service: String,
    pub tool: String,
    /// The argument object. Read from stdin when omitted.
    pub arguments: Option<String>,
    /// On the local host, the caller the service is told called it.
    #[arg(long)]
    pub caller: Option<String>,
    /// On the local host, the end user's issuer the service is told it is
    /// called for.
    #[arg(long, requires = "user", requires = "caller")]
    pub issuer: Option<String>,
    /// On the local host, the end user's subject the service is told it is
    /// called for.
    #[arg(long, requires = "issuer")]
    pub user: Option<String>,
}

/// Call the tool `call` names, as the caller it names.
pub fn run(client: &Client, call: &Call) -> Result<ExitCode> {
    let caller = call.caller.as_ref().map(|id| Caller {
        id: id.clone(),
        app: None,
        user: call
            .issuer
            .clone()
            .zip(call.user.clone())
            .map(|(issuer, subject)| User { issuer, subject }),
        client: None,
        agent: None,
    });
    tool(
        client,
        &call.service,
        &call.tool,
        call.arguments.as_deref(),
        caller.as_ref(),
    )
}

/// Call `tool` of `program`.
pub fn tool(
    client: &Client,
    program: &str,
    tool: &str,
    inline: Option<&str>,
    caller: Option<&Caller>,
) -> Result<ExitCode> {
    let arguments = arguments(inline)?;
    let (output, meter) = runtime::wait(client.call(program, tool, arguments, caller))?;
    if let Some(meter) = meter {
        eprintln!(
            "{}µs CPU, {} rows read, {} written, {}µs SQL CPU, {} KiB memory, {} bytes proxied",
            meter.cpu,
            meter.read,
            meter.written,
            meter.sql_cpu,
            meter.memory / 1024,
            meter.proxied
        );
    }
    match output {
        Output::Done(result) => {
            println!("{result}");
            Ok(ExitCode::SUCCESS)
        }
        Output::Failed(failure) => {
            eprintln!("{program}.{tool} failed: {failure}");
            Ok(ExitCode::from(FAILED))
        }
    }
}

/// What was typed, what was piped in, or nothing, which is what MCP sends for a
/// call with no arguments.
///
/// Trimmed: `echo '{}' | crab call` reaches the program as the same bytes
/// typing it does.
fn arguments(inline: Option<&str>) -> Result<Vec<u8>> {
    let raw = match inline {
        Some(arguments) => arguments.as_bytes().to_vec(),
        None => {
            let stdin = std::io::stdin();
            if stdin.is_terminal() {
                return Ok(Vec::new());
            }
            let mut piped = Vec::new();
            stdin
                .lock()
                .read_to_end(&mut piped)
                .context("cannot read arguments from stdin")?;
            piped
        }
    };
    Ok(raw.trim_ascii().to_vec())
}
