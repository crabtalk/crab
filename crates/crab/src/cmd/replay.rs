//! `crab replay` — run a call that failed again: from its tape, or against
//! another host's state.

use crate::{cmd::call, runtime};
use anyhow::{Result, bail};
use berm::{
    Berm, Calls, Config, Engine, Output, User, abi, chain,
    syscall::emit,
    tape::{Tape, Taping},
};
use berm_api::digest;
use crabtalk_sdk::client::Client;
use std::{process::ExitCode, sync::Arc};

/// Leaves by the door `crab call` takes for a tool that failed.
const FAILED: u8 = 2;

/// Run `program`'s call `id`, as `from` recorded it, in this process: the
/// image it ran, its arguments and caller, each syscall answered from its
/// tape. Its logs are printed here.
pub fn tape(from: &Client, program: &str, id: u64) -> Result<ExitCode> {
    let input = runtime::wait(from.call_input(program, id))?;
    let tape = Tape::try_from(&runtime::wait(from.call_tape(program, id))?[..])?;
    let bytes = runtime::wait(from.call_image(program, id))?;
    if input.digest != digest::of(&bytes) {
        bail!(
            "the host served an image of another digest than {}",
            input.digest
        );
    }
    if !tape.complete {
        eprintln!("the tape was cut short; the replay stops where it ends");
    }

    let engine = Engine::new(&Config::default())?;
    let logs = emit::programs(|_, log| async move {
        eprintln!("{} {}", log.topics.join(" "), log.data);
        Ok(true)
    });
    let berm = Berm::new(&engine, Calls::by_name(Calls::DEFAULT_DEPTH), vec![logs]);
    let deployed = berm.deploy(program, &bytes)?;
    let caller = input.caller.as_ref();
    let replayed = runtime::wait(
        deployed.call_as(
            caller
                .filter(|caller| !caller.anonymous())
                .map(|caller| caller.id.as_str().into()),
            caller
                .and_then(|caller| caller.user.as_ref())
                .map(|user| User {
                    issuer: user.issuer.as_str().into(),
                    subject: user.subject.as_str().into(),
                }),
            caller
                .and_then(|caller| caller.client.as_deref())
                .map(Into::into),
            &input.tool,
            input.args.into_bytes(),
            Arc::default(),
            chain::next(),
            Taping::Replay {
                tape,
                through: vec![abi::EMIT.to_owned()],
            },
        ),
    );
    match replayed {
        Ok(outcome) => match outcome.result {
            Output::Done(result) => {
                println!("{result}");
                eprintln!(
                    "{program}.{} answered; the call it replays failed",
                    input.tool
                );
                Ok(ExitCode::SUCCESS)
            }
            Output::Failed(failure) => {
                eprintln!("{program}.{} failed: {failure}", input.tool);
                Ok(ExitCode::from(FAILED))
            }
        },
        Err(error) => {
            eprintln!("{program}.{} trapped: {error:#}", input.tool);
            Ok(ExitCode::from(FAILED))
        }
    }
}

/// Run `program`'s call `id`, as `from` recorded it, on `on`: its tool, its
/// arguments and its caller, against `on`'s own state.
pub fn on(from: &Client, on: &Client, program: &str, id: u64) -> Result<ExitCode> {
    let input = runtime::wait(from.call_input(program, id))?;
    let running = runtime::wait(on.inspect(program))?.digest;
    let running = digest::prefixed(running.trim_start_matches(digest::PREFIX));
    if input.digest != running {
        eprintln!(
            "the call ran {}; {program} here runs {running}",
            input.digest
        );
    }
    call::tool(
        on,
        program,
        &input.tool,
        Some(&input.args),
        input.caller.as_ref(),
    )
}
