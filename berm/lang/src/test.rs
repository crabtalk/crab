//! Run a program's tools natively, without a toolchain or a daemon.
//!
//! A program is an image that a host compiles and enters, which is a slow way
//! to find out that a handler mishandles an empty string. Off a guest target
//! the exports are ordinary functions, so a test can call one the way the host
//! would — the same argument transfer, the same failure channel — and check
//! what came back. The host's bound on arguments and results is not applied.
//!
//! ```ignore
//! #[test]
//! fn echo_answers_its_arguments() {
//!     let out = berm_lang::test::call(berm_tool_echo, br#"{"query":"hi"}"#).unwrap();
//!     assert_eq!(out, br#"{"query":"hi"}"#);
//! }
//! ```
#![cfg(not(target_arch = "wasm32"))]

use crate::{CallError, abi, http, sys};
use std::{string::String, vec::Vec};

/// Invoke a tool export with `args`, as the host does.
///
/// `Err` is what the program reported through its failure channel — the same
/// distinction the host draws between a tool that failed and one that returned
/// the word "error".
pub fn call(entry: extern "C" fn(), args: &[u8]) -> Result<Vec<u8>, String> {
    sys::with(|host| {
        host.args = args.to_vec();
        host.traced.clear();
        host.emitted.clear();
        host.done = None;
        host.failure = None;
    });

    entry();

    // A tool that returned without answering wrote nothing, which is an empty
    // result rather than a missing one.
    sys::with(|host| match (host.failure.take(), host.done.take()) {
        (Some(failure), _) => Err(failure),
        (None, Some(result)) => Ok(result),
        (None, None) => Ok(Vec::new()),
    })
}

/// What the program traced during the last [`call`].
pub fn traced() -> Vec<String> {
    sys::with(|host| host.traced.clone())
}

/// Each log's topics and data, as the program emitted them during the last
/// [`call`].
pub fn emitted() -> Vec<(Vec<String>, String)> {
    sys::with(|host| host.emitted.clone())
}

/// Arrange what another program answers, so a tool that calls one can be run
/// without a host running the other side.
///
/// Answers persist until [`forget`], so a test sets them up once and calls as
/// often as it likes.
///
/// ```ignore
/// test::answer("weather", "forecast", Ok(br#"{"c":7}"#));
/// test::answer("gone", "any", Err(CallError::Refused("not deployed".into())));
/// ```
pub fn answer(program: &str, tool: &str, result: Result<&[u8], CallError>) {
    let (outcome, bytes) = match result {
        Ok(bytes) => (0, bytes.to_vec()),
        Err(CallError::Failed(message)) => (abi::ERROR, message.into_bytes()),
        Err(CallError::Refused(message)) => (abi::ERROR | abi::REFUSED, message.into_bytes()),
    };
    sys::with(|host| {
        host.answers.retain(|(a, b, ..)| a != program || b != tool);
        host.answers
            .push((program.into(), tool.into(), outcome, bytes));
    });
}

/// Drop every answer [`answer`] and response [`respond`] arranged, and every
/// request [`fetched`] recorded.
pub fn forget() {
    sys::with(|host| {
        host.answers.clear();
        host.responses.clear();
        host.fetched.clear();
    });
}

/// One request a program sent through `berm.http.fetch` or `berm.http.proxy`,
/// `{secret:NAME}` as the program wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Fetched {
    /// The value of the header `name`, ASCII case ignored.
    pub fn header(&self, name: &str) -> Option<&str> {
        http::header(&self.headers, name)
    }
}

/// Arrange what a request answers, by method and URL, with no headers.
///
/// Responses persist until [`forget`]. A request nothing was arranged for
/// panics naming it.
///
/// ```ignore
/// test::respond("GET", "https://api.example.com/items", 200, b"[]");
/// ```
pub fn respond(method: &str, url: &str, status: u16, body: &[u8]) {
    sys::with(|host| {
        host.responses.retain(|(m, u, ..)| m != method || u != url);
        host.responses
            .push((method.into(), url.into(), status, body.to_vec()));
    });
}

/// Each request sent since [`forget`], in order.
pub fn fetched() -> Vec<Fetched> {
    sys::with(|host| host.fetched.clone())
}
