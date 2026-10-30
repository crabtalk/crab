//! The reference guest: the smallest real service, and what berm is measured
//! and tested against.
//!
//! Each tool prices or proves one thing. `berm/berm/examples/measure.rs` and
//! `load.rs` read the numbers off them, and `tests/tools.rs` is the only
//! exercise the SDK's host-side `test::call` gets.

// Off a guest target `cargo test` runs the tools below natively.
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use berm_lang::{Caller, Result, Schema, event, tool};

berm_lang::declare_service!(fixture);

/// Answers its arguments back.
#[tool]
pub fn echo(
    /// The text to echo back.
    query: String,
    /// Page number, zero-indexed.
    page: Option<u32>,
) -> Result<Echo> {
    Ok(Echo { query, page })
}

/// Takes its arguments without reading them, to price the parse `echo`
/// makes of the same payload.
#[tool]
pub fn carry() -> Result<()> {
    Ok(())
}

/// Loops `rounds` times, to run a call past the CPU time it includes.
#[tool]
pub fn spin(rounds: u64) -> Result<u64> {
    let mut acc = 0u64;
    for round in 0..rounds {
        acc = core::hint::black_box(acc.wrapping_mul(31).wrapping_add(round));
    }
    Ok(acc)
}

/// Lowers this call's memory budget to `memory` bytes, `0` keeping it, then
/// loops `rounds` times, answering whether lowering it again was refused.
#[tool]
pub fn lowered(memory: u64, rounds: u64) -> Result<bool> {
    berm_lang::limit(0, 0, 0, memory)?;
    let refused = berm_lang::limit(0, 0, 0, memory).is_err();
    core::hint::black_box(spin(rounds)?);
    Ok(refused)
}

/// Makes 100 host calls, to price one.
#[tool]
pub fn chatty() -> Result<()> {
    let mut total = 0;
    for _ in 0..100 {
        total += berm_lang::args_len();
    }
    if total == usize::MAX {
        return Err("impossible".into());
    }
    Ok(())
}

/// Allocates, to prove the heap arrives without a second entry.
#[tool]
pub fn probe() -> Result<Vec<u8>> {
    let v = alloc::vec![7u8; 4096];
    Ok(v[..2].to_vec())
}

/// Reads one stored key and answers whether it was set. Under
/// `examples/load.rs` the read stands in for a host call that waits on
/// I/O.
#[tool]
pub fn wait() -> Result<Wait> {
    Ok(match berm_lang::get("wait")? {
        Some(_) => Wait::Set,
        None => Wait::Unset,
    })
}

/// Fetches `url`, posting `body` when there is one, and answers the status
/// and the body's length.
#[tool]
pub fn fetch(
    url: String,
    /// One `Name: Value` header, which may name `{secret:fixture}` or
    /// `{secret:spare}`.
    header: Option<String>,
    /// What to post, which may name `{secret:fixture}` or `{secret:spare}`.
    body: Option<String>,
) -> Result<Fetched> {
    let header = header.as_deref().and_then(|header| {
        header
            .split_once(':')
            .map(|(name, value)| (name.trim(), value.trim()))
    });
    let response = match &body {
        Some(body) => berm_lang::http::fetch("POST", &url, header.as_slice(), body.as_bytes())?,
        None => berm_lang::http::fetch("GET", &url, header.as_slice(), b"")?,
    };
    Ok(Fetched {
        status: response.status,
        bytes: response.body.len() as u64,
    })
}

/// Answers 32 bytes from the host's random source, as hex.
#[tool]
pub fn random() -> Result<String> {
    let mut bytes = [0; 32];
    berm_lang::random(&mut bytes);
    Ok(hex::encode(bytes))
}

/// Answers its copy's `greeting` var.
#[tool]
pub fn greet() -> Result<String> {
    Ok(berm_lang::var("greeting")?)
}

/// Reads a var `[vars]` does not declare.
#[tool]
pub fn undeclared() -> Result<String> {
    Ok(berm_lang::var("missing")?)
}

/// Says who called: `outside NAME`, `service NAME`, or `none`, then
/// ` user ISSUER SUBJECT` when the chain runs for a user.
#[tool]
pub fn whoami() -> Result<String> {
    let mut said = match berm_lang::caller()? {
        Some(Caller::Outside(name)) => format!("outside {name}"),
        Some(Caller::Service(name)) => format!("service {name}"),
        None => String::from("none"),
    };
    if let Some(user) = berm_lang::user()? {
        said.push_str(&format!(" user {} {}", user.issuer, user.subject));
    }
    Ok(said)
}

/// Answers `ok` to a service, and denies any other caller.
#[tool]
pub fn guarded() -> Result<String> {
    match berm_lang::caller()? {
        Some(Caller::Service(_)) => Ok("ok".into()),
        _ => Err(berm_lang::deny("services only")),
    }
}

/// Answers `ok`; only services may call it.
#[tool]
pub fn fromservices() -> Result<String> {
    Ok("ok".into())
}

/// Calls `guarded` and `fromservices` on `fixture`, and answers what they
/// said.
#[tool]
pub fn askguarded() -> Result<String> {
    use crate::fixture::calls;

    Ok(format!("{} {}", calls::guarded()?, calls::fromservices()?))
}

/// Calls `whoami` on `fixture`, and answers what it said.
#[tool]
pub fn askwho() -> Result<String> {
    Ok(crate::fixture::calls::whoami()?)
}

/// Vouches for `subject`, then calls `whoami` on `fixture`, and answers
/// what it said.
#[tool]
pub fn vouched(
    /// Who the chain runs for from here on.
    subject: String,
) -> Result<String> {
    berm_lang::vouch(&subject)?;
    Ok(crate::fixture::calls::whoami()?)
}

/// Makes the host calls no other tool here makes, whatever each answers, so
/// this image imports them. Answers `url`'s fetch status, when it had one.
#[tool]
pub fn reach(url: String) -> Result<Option<u16>> {
    let request = berm_lang::http::Request {
        method: "GET",
        url: &url,
        headers: &[],
        body: b"",
        timeout: Some(1_000),
        limit: None,
    };
    let _ = berm_lang::client();
    let _ = berm_lang::http::proxy("GET", &url, &[], b"");
    let _ = berm_lang::http::proxy_all(core::slice::from_ref(&request));
    let fetched = berm_lang::http::fetch_all(core::slice::from_ref(&request))?;
    Ok(fetched
        .into_iter()
        .next()
        .and_then(|fetched| fetched.ok())
        .map(|response| response.status))
}

/// Always fails, to exercise the error path.
#[tool]
pub fn boom() -> Result<()> {
    Err("boom, as requested".into())
}

/// Fetches `url`, draws 4 random bytes, then fails with the body it got and
/// the bytes, to exercise a tape.
#[tool]
pub fn flaky(url: String) -> Result<()> {
    let response = berm_lang::http::fetch("GET", &url, &[], b"")?;
    let mut bytes = [0; 4];
    berm_lang::random(&mut bytes);
    Err(format!(
        "upstream said {} ({})",
        String::from_utf8_lossy(&response.body),
        hex::encode(bytes)
    )
    .into())
}

/// Calls `echo` on whatever is deployed as `inner`, and says which kind of
/// failure came back.
#[tool]
pub fn nest(query: String, page: Option<u32>) -> Result<Echo> {
    let args = berm_lang::json::to(&Echo { query, page })?;
    match berm_lang::call("inner", "echo", &args) {
        Ok(result) => Ok(berm_lang::json::from(&result)?),
        Err(error) if error.refused() => Err(format!("refused: {}", error.message()).into()),
        Err(error) => Err(error.into()),
    }
}

/// Counts how many times it has run, arming itself to run again in `delay`
/// milliseconds until it has run three times. Proves an invocation can
/// outlive the one that asked for it, and that what crosses the gap is the
/// service's own keys.
#[tool]
pub fn tick(delay: u64) -> Result<Round> {
    let round = berm_lang::get("ticks")
        .ok()
        .flatten()
        .and_then(|seen| core::str::from_utf8(&seen).ok()?.parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    berm_lang::set("ticks", format!("{round}").as_bytes())?;

    // The delay rides along, so each round arms the next on the same one.
    if round < 3 {
        berm_lang::after(delay, "fixture", "tick", &format!(r#"{{"delay":{delay}}}"#))?;
    }
    Ok(Round {
        round,
        at: berm_lang::now(),
    })
}

/// Writes `word` under `note`, emits it as a `Noted` event, answers what it
/// reads back, and fails when `word` is `fail`.
#[tool]
pub fn note(word: String) -> Result<String> {
    berm_lang::set("note", word.as_bytes())?;
    berm_lang::emit!(Noted {
        bytes: word.len() as u64,
        word: word.clone(),
    })?;
    if word == "fail" {
        return Err("failed, as requested".into());
    }
    let seen = berm_lang::get("note")?.ok_or("note was not kept")?;
    Ok(String::from_utf8_lossy(&seen).into_owned())
}

/// Calls `note` on `inner` with `word`, carries on whether or not that
/// failed, emits `word` under the topic `relay`, and answers what this
/// service reads back under `note`.
#[tool]
pub fn relay(word: String) -> Result<String> {
    let args = format!(r#"{{"word":{}}}"#, berm_lang::json::to(&word)?);
    let _ = berm_lang::call("inner", "note", &args);
    berm_lang::emit(&["relay"], &word)?;
    Ok(match berm_lang::get("note")? {
        Some(seen) => String::from_utf8_lossy(&seen).into_owned(),
        None => String::from("unset"),
    })
}

/// Calls `peek` on `fixture`, then `echo` on `fixture` with the note it
/// read, and answers the query `echo` answered.
#[tool]
pub fn ask() -> Result<String> {
    use crate::fixture::calls;

    let peek = calls::peek()?;
    let echoed = calls::echo(None, peek.note.unwrap_or_default())?;
    Ok(echoed.query)
}

/// Reads `note`, then tries to write it, emit under `note`, arm `tick` and
/// call `note` on `inner`, and answers what it read and what each attempt
/// was told.
#[tool(query)]
pub fn peek() -> Result<Peek> {
    let told = |attempt: core::result::Result<(), berm_lang::CallError>| match attempt {
        Ok(()) => String::from("allowed"),
        Err(error) => error.message().into(),
    };
    let note = berm_lang::get("note")
        .ok()
        .flatten()
        .map(|value| String::from_utf8_lossy(&value).into_owned());
    Ok(Peek {
        note,
        attempts: Attempts {
            set: told(berm_lang::set("note", b"peeked")),
            emit: told(berm_lang::emit(&["note"], "peeked")),
            after: told(berm_lang::after(0, "fixture", "tick", r#"{"delay":0}"#)),
            call: told(berm_lang::call("inner", "note", r#"{"word":"peeked"}"#).map(|_| ())),
        },
    })
}

/// Calls itself on `inner` one level deeper, which is this same service
/// when deployed under that name — the runaway a depth limit exists to
/// stop. Answers how many levels got through before the host refused.
#[tool]
pub fn recurse(depth: u32) -> Result<u32> {
    let deeper = depth + 1;
    match berm_lang::call("inner", "recurse", &format!(r#"{{"depth":{deeper}}}"#)) {
        Ok(reached) => Ok(berm_lang::json::from(&reached)?),
        // The bottom of the chain: the host refused to go further, and the
        // depth reached rides back up as the result.
        Err(error) if error.refused() => Ok(deeper),
        Err(error) => Err(error.into()),
    }
}

/// What `echo` answers.
#[derive(Schema)]
pub struct Echo {
    pub query: String,
    pub page: Option<u32>,
}

/// What `wait` answers.
#[derive(Schema)]
pub enum Wait {
    Set,
    Unset,
}

/// What `fetch` answers.
#[derive(Schema)]
pub struct Fetched {
    pub status: u16,
    /// The body's length.
    pub bytes: u64,
}

/// What `tick` answers.
#[derive(Schema)]
pub struct Round {
    /// How many times it has run, this time included.
    pub round: u64,
    /// When, in Unix milliseconds.
    pub at: u64,
}

/// A word `note` wrote.
#[event]
pub struct Noted {
    pub word: String,
    /// Its length in bytes.
    pub bytes: u64,
}

/// What `peek` answers.
#[derive(Schema)]
pub struct Peek {
    /// What `note` held, if anything.
    pub note: Option<String>,
    pub attempts: Attempts,
}

/// What each of `peek`'s attempts was told.
#[derive(Schema)]
pub struct Attempts {
    pub set: String,
    pub emit: String,
    pub after: String,
    pub call: String,
}
