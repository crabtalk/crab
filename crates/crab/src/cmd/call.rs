//! `crab call` — call one of a service's tools.

use crate::{caller::Identity, format, runtime, terminal};
use anyhow::Result;
use berm_api::{Output, host::Caller};
use clap::Args;
use crabtalk_sdk::client::Client;
use std::process::ExitCode;

/// A service that ran and said no, which is not berm failing to run it — the
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
    #[command(flatten)]
    pub identity: Identity,
}

/// Call the tool `call` names, priced at `cloud`'s rates.
pub fn run(client: &Client, cloud: &Client, call: &Call) -> Result<ExitCode> {
    let caller = call.identity.resolve(client)?;
    tool(
        client,
        cloud,
        &call.service,
        &call.tool,
        call.arguments.as_deref(),
        caller.as_ref(),
    )
}

/// Call `tool` of `service`, and print what its run used and, when `cloud`
/// answers with its rates, what it cost to run.
pub fn tool(
    client: &Client,
    cloud: &Client,
    service: &str,
    tool: &str,
    inline: Option<&str>,
    caller: Option<&Caller>,
) -> Result<ExitCode> {
    let arguments = terminal::arguments(inline)?;
    let (called, metering) = runtime::wait(async {
        tokio::join!(client.call(service, tool, arguments, caller), cloud.rates())
    });
    let (output, meter) = called?;
    if let Some(meter) = meter {
        let cost = metering.map_or_else(
            |_| String::new(),
            |metering| {
                let overage = meter.cpu.saturating_sub(metering.included_cpu);
                let micros = metering.rates.cost_decimal(&meter, overage);
                format!(", {} to run", format::usd(micros.parse().unwrap_or(0.0)))
            },
        );
        eprintln!(
            "{}µs CPU, {} rows read, {} written, {}µs SQL CPU, {} KiB memory, {} bytes proxied{cost}",
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
            eprintln!("{service}.{tool} failed: {failure}");
            Ok(ExitCode::from(FAILED))
        }
    }
}
