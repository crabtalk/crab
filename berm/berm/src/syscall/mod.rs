//! What a guest can reach.
//!
//! A syscall is native host code behind a name. berm serves `berm.call`
//! itself. The standard calls — [`store`], [`emit`], [`after`], [`http`],
//! [`sql`], [`var`] — are read and checked here and handed as typed values to closures
//! the embedder registers; anything else is a [`crate::Syscall`] the embedder
//! writes whole. All of them are reached through one table.

use crate::{
    Call, Caller as Asking, Callsite, Invocation, Refused, Syscall, User, abi, engine, tape::Answer,
};
use anyhow::{Result, anyhow, bail};
use ring::rand::{SecureRandom, SystemRandom};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};
use wasmtime::Caller;

pub mod after;
pub(crate) mod call;
pub mod emit;
pub mod http;
pub mod sql;
pub mod store;
pub mod var;

/// One of the ABI's own doors: request bytes at `(ptr, len)` in, a staged
/// length out, answered before the guest resumes.
///
/// A call that takes no arguments is handed a pair it ignores.
pub(crate) type Handler =
    Arc<dyn Fn(&mut Caller<'_, Invocation>, u64, u64) -> Result<u64> + Send + Sync>;

/// What answers one syscall number.
#[derive(Clone)]
pub(crate) enum Door {
    /// The ABI's own, which touch nothing but the guest.
    Builtin(Handler),
    /// An embedder's, handed the request and awaited.
    Hosted(Call),
}

/// Every syscall one service can reach, by the number its name hashes to.
pub(crate) type Table = HashMap<u64, Door>;

/// Assemble what a service is instantiated with: the ABI's own doors, then
/// whatever the embedder registered.
///
/// A name already spoken for, berm's own included, is refused.
pub(crate) fn table(syscalls: &[Syscall]) -> Result<Table> {
    let mut table = builtins();
    for syscall in syscalls {
        let number = abi::hash(&syscall.name);
        if table.contains_key(&number) {
            bail!("syscall {:?} is already served", syscall.name);
        }
        table.insert(number, Door::Hosted(syscall.call.clone()));
    }
    Ok(table)
}

/// Whether `number` is one of the ABI's own doors.
pub(crate) fn is_builtin(number: u64) -> bool {
    builtins().contains_key(&number)
}

/// Copy a hosted syscall's request out of the guest, beside the callsite it
/// came from.
pub(crate) fn request(
    guest: &mut Caller<'_, Invocation>,
    ptr: u64,
    len: u64,
) -> Result<(Callsite, Vec<u8>)> {
    let request = engine::read(guest, ptr, len)?;
    Ok((guest.data().at.clone(), request))
}

/// Leave a syscall's bytes for the guest to pull, and answer with their length.
///
/// The [`abi::ERROR`] bit says the staged bytes are a failure message. A
/// [`Refused`] also sets [`abi::REFUSED`]: the call never ran.
pub(crate) fn stage(guest: &mut Caller<'_, Invocation>, answer: Result<Vec<u8>>) -> u64 {
    let (staged, outcome) = match answer {
        Ok(result) => (result, 0),
        Err(error) => {
            let refused = error
                .chain()
                .any(|cause| cause.downcast_ref::<Refused>().is_some());
            let outcome = match refused {
                true => abi::ERROR | abi::REFUSED,
                false => abi::ERROR,
            };
            (error.to_string().into_bytes(), outcome)
        }
    };
    let length = staged.len() as u64;
    guest.data_mut().staged = staged;
    length | outcome
}

/// The doors the ABI itself owns: the argument blob in, the result out, and
/// the few things a guest cannot learn any other way.
fn builtins() -> Table {
    let mut table = Table::new();

    table.insert(
        abi::HOST_TRACE,
        handler(|guest, ptr, len| {
            let message = engine::read(guest, ptr, len)?;
            tracing::info!(target: "service", "{}", String::from_utf8_lossy(&message));
            Ok(0)
        }),
    );

    table.insert(
        abi::HOST_CALLER,
        handler(|guest, _, _| {
            let reply = match &guest.data().at.caller {
                Some(Asking::Outside(name)) => abi::wire::request(&[b"outside", name.as_bytes()]),
                Some(Asking::Service(name)) => abi::wire::request(&[b"service", name.as_bytes()]),
                None => Vec::new(),
            };
            Ok(stage(guest, Ok(reply)))
        }),
    );

    table.insert(
        abi::HOST_USER,
        handler(|guest, _, _| {
            let reply = match &guest.data().at.user {
                Some(user) => {
                    abi::wire::request(&[user.issuer.as_bytes(), user.subject.as_bytes()])
                }
                None => Vec::new(),
            };
            Ok(stage(guest, Ok(reply)))
        }),
    );

    table.insert(
        abi::HOST_VOUCH,
        handler(|guest, ptr, len| {
            let subject = engine::read(guest, ptr, len)?;
            let answer = vouch(guest.data_mut(), subject);
            Ok(stage(guest, answer))
        }),
    );

    table.insert(
        abi::HOST_CLIENT,
        handler(|guest, _, _| {
            let reply = match &guest.data().at.client {
                Some(client) => abi::wire::request(&[client.as_bytes()]),
                None => Vec::new(),
            };
            Ok(stage(guest, Ok(reply)))
        }),
    );

    table.insert(
        abi::HOST_DENY,
        handler(|guest, _, _| {
            guest.data_mut().denied = true;
            Ok(0)
        }),
    );

    table.insert(
        abi::HOST_LIMIT,
        handler(|guest, ptr, len| {
            let bytes = engine::read(guest, ptr, len)?;
            let answer = limit(guest.data_mut(), &bytes);
            Ok(stage(guest, answer))
        }),
    );

    table.insert(
        abi::HOST_ARG_LEN,
        handler(|guest, _, _| Ok(guest.data().args.len() as u64)),
    );

    // Answers with the blob's full length, which exceeds `capacity` when the
    // copy was truncated.
    table.insert(
        abi::HOST_ARG_READ,
        handler(|guest, ptr, capacity| {
            let args = std::mem::take(&mut guest.data_mut().args);
            let length = args.len();
            let written = engine::write(guest, ptr, &args[..length.min(capacity as usize)]);
            guest.data_mut().args = args;
            written?;
            Ok(length as u64)
        }),
    );

    // Registered for every service, including one given no syscalls.
    table.insert(
        abi::HOST_RESULT_READ,
        handler(|guest, ptr, capacity| {
            let staged = std::mem::take(&mut guest.data_mut().staged);
            let length = staged.len();
            let written = engine::write(guest, ptr, &staged[..length.min(capacity as usize)]);
            guest.data_mut().staged = staged;
            written?;
            Ok(length as u64)
        }),
    );

    table.insert(
        abi::HOST_DONE,
        handler(|guest, ptr, len| {
            let result = engine::read(guest, ptr, len)?;
            guest.data_mut().outcome = Some(Ok(result));
            Ok(0)
        }),
    );

    table.insert(
        abi::HOST_FAIL,
        handler(|guest, ptr, len| {
            let message = engine::read(guest, ptr, len)?;
            let message = String::from_utf8_lossy(&message).into_owned();
            guest.data_mut().outcome = Some(Err(message));
            Ok(0)
        }),
    );

    table.insert(
        abi::HOST_NOW,
        handler(|guest, _, _| {
            let Some(deck) = guest.data().tape.clone() else {
                return Ok((guest.data().now)());
            };
            let replayed = deck.lock().replay(&guest.data().at, abi::HOST_NOW, [0; 32]);
            match replayed {
                Some(Ok(Answer::Number(now))) => Ok(now),
                Some(Ok(_)) => bail!("the tape answered berm.now with no reading"),
                Some(Err(diverged)) => Err(diverged),
                None => {
                    let now = (guest.data().now)();
                    deck.lock()
                        .record(abi::HOST_NOW, [0; 32], Answer::Number(now));
                    Ok(now)
                }
            }
        }),
    );

    table.insert(
        abi::HOST_RANDOM,
        handler(|guest, ptr, len| {
            let deck = guest.data().tape.clone();
            let replayed = deck.as_ref().and_then(|deck| {
                deck.lock()
                    .replay(&guest.data().at, abi::HOST_RANDOM, [0; 32])
            });
            let filled = engine::slice_mut(guest, ptr, len)?;
            match replayed {
                Some(Ok(Answer::Done(bytes))) if bytes.len() == filled.len() => {
                    filled.copy_from_slice(&bytes)
                }
                Some(Ok(_)) => bail!("the tape answered berm.random with another length"),
                Some(Err(diverged)) => return Err(diverged),
                None => {
                    SystemRandom::new()
                        .fill(filled)
                        .map_err(|_| anyhow!("the system's random source failed"))?;
                    if let Some(deck) = deck {
                        let bytes = filled.to_vec();
                        deck.lock()
                            .record(abi::HOST_RANDOM, [0; 32], Answer::Done(bytes));
                    }
                }
            }
            Ok(0)
        }),
    );

    table
}

/// Lower `invocation`'s budget to what `bytes` holds, as [`abi::HOST_LIMIT`]
/// reads it.
fn limit(invocation: &mut Invocation, bytes: &[u8]) -> Result<Vec<u8>> {
    let [cpu, read, written, memory]: [u64; 4] = match bytes.len() {
        32 => std::array::from_fn(|i| {
            u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().expect("eight bytes"))
        }),
        _ => return Err(Refused(String::from("a limit is four u64s")).into()),
    };
    if invocation.limited {
        return Err(Refused(String::from("a call lowers its budget once")).into());
    }
    let own = invocation.at.own.clone();
    let held_cpu = invocation.cpu.load(Ordering::Relaxed);
    let (held_read, held_written) = own.budget();
    if cpu > held_cpu || read > held_read || written > held_written {
        return Err(Refused(format!(
            "a call lowers its budget of {held_cpu}µs of CPU, {held_read} rows read and \
             {held_written} rows written, never raises it"
        ))
        .into());
    }
    if memory != 0 {
        invocation.held.lower(memory)?;
    }
    if cpu != 0 {
        invocation.cpu.store(cpu, Ordering::Relaxed);
    }
    own.lower(read, written);
    invocation.limited = true;
    Ok(Vec::new())
}

/// Have the rest of `invocation`'s chain run for `subject`, vouched for by its
/// service.
fn vouch(invocation: &mut Invocation, subject: Vec<u8>) -> Result<Vec<u8>> {
    let issuer = invocation.issuer.clone();
    let at = &mut invocation.at;
    if !at.deps.syscalls.contains_key(abi::VOUCH) {
        return Err(Refused(format!("{} does not declare {}", at.service, abi::VOUCH)).into());
    }
    if at.user.is_some() {
        return Err(Refused(String::from("the chain already runs for a user")).into());
    }
    let subject =
        String::from_utf8(subject).map_err(|_| Refused(String::from("a subject is UTF-8 text")))?;
    if subject.is_empty() {
        return Err(Refused(String::from("a subject names someone")).into());
    }
    at.user = Some(User {
        issuer,
        subject: subject.into(),
    });
    Ok(Vec::new())
}

fn handler(
    call: impl Fn(&mut Caller<'_, Invocation>, u64, u64) -> Result<u64> + Send + Sync + 'static,
) -> Door {
    Door::Builtin(Arc::new(call))
}
