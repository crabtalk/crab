//! Calling a tool on another program.
//!
//! The target is named at the call. What is deployed beside the program is
//! reachable; a name that is not answers [`CallError::Refused`].

use crate::{
    abi::{self, wire},
    host,
};
use alloc::string::{String, ToString};

pub use host::CallError;

/// Run `tool` on the program deployed as `program`, with `args` as its argument
/// blob — the same JSON a model would have sent it.
///
/// ```ignore
/// let result = berm_lang::call("weather", "forecast", r#"{"city":"Oslo"}"#)?;
/// ```
///
/// `Err` says which kind of failure it was. [`CallError::Refused`] means
/// nothing ran — no such program, or the chain is already as deep as the host
/// allows — and [`CallError::Failed`] means the tool ran and reported failure.
pub fn call(program: &str, tool: &str, args: &str) -> Result<String, CallError> {
    let request = wire::request(&[program.as_bytes(), tool.as_bytes(), args.as_bytes()]);
    let result = host::call(abi::HOST_CALL, &request)?;
    Ok(String::from_utf8_lossy(&result).into_owned())
}

/// Have `tool` on `program` run in `delay` milliseconds, and return now.
///
/// ```ignore
/// berm_lang::after(300, "weather", "poll", "{}")?;
/// ```
///
/// One wake per program: arming again drops whatever this one had pending. The
/// invocation it starts carries nothing from this one; what the woken tool
/// needs goes in [`crate::set`]. Its failure is logged by the host and reaches
/// no one; the program and tool are checked when it is armed.
pub fn after(delay: u64, program: &str, tool: &str, args: &str) -> Result<(), CallError> {
    let request = wire::request(&[
        delay.to_string().as_bytes(),
        program.as_bytes(),
        tool.as_bytes(),
        args.as_bytes(),
    ]);
    host::call(abi::HOST_CALL_AFTER, &request)?;
    Ok(())
}

/// Who made a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// Someone the host authenticated: an account, as the host names it.
    Outside(String),
    /// Another program, through [`call`], as the host names it.
    Program(String),
}

/// Who made this call, as the host says. `None` when the host started it on
/// this program's own behalf: a wake.
///
/// Set by the host, never by the caller:
///
/// ```ignore
/// if berm_lang::caller()? != Some(Caller::Program("lake".into())) {
///     return Err(berm_lang::deny("only lake calls this"));
/// }
/// ```
pub fn caller() -> Result<Option<Caller>, CallError> {
    let reply = host::call(abi::HOST_CALLER, &[])?;
    let unreadable =
        || CallError::Failed(String::from("host framed a reply this guest cannot read"));
    let fields = wire::fields(&reply).map_err(|_| unreadable())?;
    match fields.as_slice() {
        [] => Ok(None),
        [kind, name] => {
            let name = String::from_utf8_lossy(name).into_owned();
            match *kind {
                b"outside" => Ok(Some(Caller::Outside(name))),
                b"program" => Ok(Some(Caller::Program(name))),
                _ => Err(unreadable()),
            }
        }
        _ => Err(unreadable()),
    }
}

/// An end user, as the host verified them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// Who vouched for them: an identity provider's issuer URL, or the
    /// program that called [`vouch`], as the programs it calls are told it is
    /// named.
    pub issuer: String,
    /// Who they are to that issuer.
    pub subject: String,
}

/// The end user this chain runs for, as the host verified them, carried into
/// every nested call. [`caller`] says who made this call, which in a nested
/// call is the program that relayed the user.
pub fn user() -> Result<Option<User>, CallError> {
    let reply = host::call(abi::HOST_USER, &[])?;
    let unreadable =
        || CallError::Failed(String::from("host framed a reply this guest cannot read"));
    match wire::fields(&reply).map_err(|_| unreadable())?.as_slice() {
        [] => Ok(None),
        [issuer, subject] => Ok(Some(User {
            issuer: String::from_utf8_lossy(issuer).into_owned(),
            subject: String::from_utf8_lossy(subject).into_owned(),
        })),
        _ => Err(unreadable()),
    }
}

/// Run the rest of this invocation, and every call it makes after, for the end
/// user `subject`, with this program as the issuer [`user`] answers.
///
/// ```ignore
/// let account = verified_session(&token)?;
/// berm_lang::vouch(&account)?;
/// let plan = berm_lang::call("billing", "plan", "{}")?;
/// ```
///
/// The manifest declares `berm.vouch`. [`CallError::Refused`] in a chain that
/// already runs for a user, and for an empty subject.
pub fn vouch(subject: &str) -> Result<(), CallError> {
    host::call(abi::HOST_VOUCH, subject.as_bytes())?;
    Ok(())
}

/// Lower this call's budget for this call alone, once, from now on: `cpu` in
/// microseconds, `rows` read and written together, `memory` in bytes, `0`
/// keeping each. Memory is billed at the budget it holds:
///
/// ```ignore
/// berm_lang::limit(0, 0, 4 << 20)?;
/// ```
///
/// [`CallError::Refused`] a second time in one call, past the budget, and
/// below the memory the call holds already.
pub fn limit(cpu: u64, rows: u64, memory: u64) -> Result<(), CallError> {
    let mut bytes = [0u8; 24];
    for (slot, value) in bytes
        .as_chunks_mut::<8>()
        .0
        .iter_mut()
        .zip([cpu, rows, memory])
    {
        *slot = value.to_le_bytes();
    }
    host::call(abi::HOST_LIMIT, &bytes)?;
    Ok(())
}

/// The outside client this chain runs for, as an opaque key that names it for
/// one day, carried into every nested call. `None` when the host supplied none.
pub fn client() -> Result<Option<String>, CallError> {
    let reply = host::call(abi::HOST_CLIENT, &[])?;
    let unreadable =
        || CallError::Failed(String::from("host framed a reply this guest cannot read"));
    match wire::fields(&reply).map_err(|_| unreadable())?.as_slice() {
        [] => Ok(None),
        [key] => Ok(Some(String::from_utf8_lossy(key).into_owned())),
        _ => Err(unreadable()),
    }
}

/// Refuse this call's caller with `message`: the tool fails, and the host
/// counts the call as denied.
pub fn deny(message: &str) -> crate::Error {
    host::deny();
    message.into()
}
