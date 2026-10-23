//! `crab replay` — run a call that failed again: from its tape, or against
//! another host's state.

use crate::{cmd::call, utils};
use anyhow::{Context, Result, bail};
use berm::{
    Berm, Calls, Config, Engine, Output, User, abi, chain, digest, storage,
    syscall::{call::DEFAULT_CALL_DEPTH, emit},
    tape::{Tape, Taping},
};
use berm_oci::{Access, Reference, Registry};
use crabtalk_sdk::client::Client;
use std::{process::ExitCode, sync::Arc};

/// Leaves by the door `crab call` takes for a tool that failed.
const FAILED: u8 = 2;

/// Run `program`'s call `id`, as `from` recorded it, in this process: the
/// image it ran, its arguments and caller, each syscall answered from its
/// tape. Its logs are printed here.
pub fn tape(from: &Client, program: &str, id: u64) -> Result<ExitCode> {
    let input = utils::wait(from.call_input(program, id))?;
    let tape = Tape::try_from(&utils::wait(from.call_tape(program, id))?[..])?;
    let image = input
        .image
        .context("the service was deployed from no registry, so its image cannot be pulled")?;
    let reference: Reference = image.parse()?;
    let bytes = Registry::open(&reference, Access::Read)?.pull(&reference.reference)?;
    if input.digest.strip_prefix("sha256:") != Some(digest(&bytes).as_str()) {
        bail!("{image} pulled an image of another digest");
    }
    if !tape.complete {
        eprintln!("the tape was cut short; the replay stops where it ends");
    }

    let engine = Engine::new(&Config::default())?;
    let logs = emit::programs(|_, log| async move {
        eprintln!("{} {}", log.topics.join(" "), log.data);
        Ok(true)
    });
    let berm = Berm::new(
        &engine,
        Calls::by_name(DEFAULT_CALL_DEPTH),
        vec![logs],
        storage::Memory::new(),
    );
    let deployed = berm.deploy(program, &bytes)?;
    let caller = input.caller.as_ref();
    let replayed = utils::wait(
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
            chain(),
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
