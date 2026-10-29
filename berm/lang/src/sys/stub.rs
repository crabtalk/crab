//! What a host call does when there is no host.
//!
//! Off wasm, the argument blob, the trace, emitted logs, the failure channel,
//! other services and HTTP are served from [`crate::test`]'s state. Any other
//! syscall panics naming it.

use crate::{abi, test::Fetched};
use std::{
    cell::RefCell,
    string::{String, ToString},
    vec::Vec,
};

std::thread_local! {
    static HOST: RefCell<Host> = const { RefCell::new(Host::new()) };
}

/// The bits of a host a test can stand in for.
pub(crate) struct Host {
    pub args: Vec<u8>,
    pub traced: Vec<String>,
    /// Each emitted log's topics and data.
    pub emitted: Vec<(Vec<String>, String)>,
    /// What the tool answered with, whichever door it used.
    pub done: Option<Vec<u8>>,
    pub failure: Option<String>,
    /// What the last syscall answered with, staged as a real host stages it.
    pub result: Vec<u8>,
    /// What a test has arranged another service to answer: the service, the
    /// tool, the outcome bits, and the bytes.
    pub answers: Vec<(String, String, u64, Vec<u8>)>,
    /// What a test has arranged a request to answer: the method, the URL,
    /// the status, and the body.
    pub responses: Vec<(String, String, u16, Vec<u8>)>,
    /// Each request sent, in order.
    pub fetched: Vec<Fetched>,
}

impl Host {
    const fn new() -> Self {
        Self {
            args: Vec::new(),
            traced: Vec::new(),
            emitted: Vec::new(),
            done: None,
            failure: None,
            result: Vec::new(),
            answers: Vec::new(),
            responses: Vec::new(),
            fetched: Vec::new(),
        }
    }
}

/// Run `f` against the thread's stand-in host.
pub(crate) fn with<T>(f: impl FnOnce(&mut Host) -> T) -> T {
    HOST.with(|host| f(&mut host.borrow_mut()))
}

#[inline]
pub fn call0(number: u64) -> u64 {
    match number {
        abi::HOST_ARG_LEN => with(|host| host.args.len() as u64),
        _ => no_host(number),
    }
}

#[inline]
pub fn call2(number: u64, a0: u64, a1: u64) -> u64 {
    match number {
        abi::HOST_ARG_READ => with(|host| {
            let taken = host.args.len().min(a1 as usize);
            // Safety: a0/a1 describe a buffer the caller owns, exactly as the
            // real host requires. Off-target that buffer is ordinary memory.
            unsafe { core::ptr::copy_nonoverlapping(host.args.as_ptr(), a0 as *mut u8, taken) };
            host.args.len() as u64
        }),
        abi::HOST_TRACE => {
            with(|host| host.traced.push(read(a0, a1)));
            0
        }
        abi::HOST_EMIT => {
            let request = bytes(a0, a1);
            let Ok(fields) = abi::wire::fields(request) else {
                no_host(number)
            };
            let mut fields = fields
                .iter()
                .map(|field| String::from_utf8_lossy(field).into_owned());
            let data = fields.next().unwrap_or_default();
            with(|host| {
                host.emitted.push((fields.collect(), data));
                host.result.clear();
            });
            0
        }
        abi::HOST_DONE => {
            with(|host| host.done = Some(bytes(a0, a1).to_vec()));
            0
        }
        abi::HOST_FAIL => {
            with(|host| host.failure = Some(read(a0, a1)));
            0
        }
        abi::HOST_RESULT_READ => with(|host| {
            let taken = host.result.len().min(a1 as usize);
            // Safety: as HOST_ARG_READ above -- the caller owns the buffer it
            // described, and off-target it is ordinary memory.
            unsafe { core::ptr::copy_nonoverlapping(host.result.as_ptr(), a0 as *mut u8, taken) };
            host.result.len() as u64
        }),
        // Another service, if the test arranged what it answers.
        abi::HOST_CALL => {
            let request = bytes(a0, a1);
            let Ok(fields) = abi::wire::fields(request) else {
                no_host(number)
            };
            let (service, tool) = match fields.as_slice() {
                [service, tool, ..] => (
                    String::from_utf8_lossy(service).into_owned(),
                    String::from_utf8_lossy(tool).into_owned(),
                ),
                _ => no_host(number),
            };
            let staged = with(|host| {
                host.answers
                    .iter()
                    .find(|(a, b, ..)| *a == service && *b == tool)
                    .map(|(.., outcome, bytes)| (*outcome, bytes.clone()))
            });
            match staged {
                Some((outcome, bytes)) => with(|host| {
                    let length = bytes.len() as u64;
                    host.result = bytes;
                    length | outcome
                }),
                None => panic!(
                    "nothing arranged for {service}.{tool}; call berm_lang::test::answer first"
                ),
            }
        }
        abi::HOST_HTTP_FETCH | abi::HOST_HTTP_PROXY => {
            let request = bytes(a0, a1);
            let Some((status, body)) = respond(request) else {
                no_host(number)
            };
            with(|host| {
                host.result = abi::wire::request(&[status.to_string().as_bytes(), &body]);
                host.result.len() as u64
            })
        }
        abi::HOST_HTTP_FETCH_ALL | abi::HOST_HTTP_PROXY_ALL => {
            let batch = bytes(a0, a1);
            let Ok(requests) = abi::wire::fields(batch) else {
                no_host(number)
            };
            let answers: Vec<Vec<u8>> = requests
                .into_iter()
                .map(|request| {
                    let Some((status, body)) = respond(request) else {
                        no_host(number)
                    };
                    abi::wire::request(&[b"ok", status.to_string().as_bytes(), &body])
                })
                .collect();
            let fields: Vec<&[u8]> = answers.iter().map(Vec::as_slice).collect();
            with(|host| {
                host.result = abi::wire::request(&fields);
                host.result.len() as u64
            })
        }
        _ => no_host(number),
    }
}

/// The status and body arranged for one framed HTTP request, kept as
/// fetched; `None` when it is not framed as one.
fn respond(request: &[u8]) -> Option<(u16, Vec<u8>)> {
    let fields = abi::wire::fields(request).ok()?;
    let [method, url, body, _timeout, limit, headers @ ..] = fields.as_slice() else {
        return None;
    };
    let text = |field: &[u8]| String::from_utf8_lossy(field).into_owned();
    let fetched = Fetched {
        method: text(method),
        url: text(url),
        headers: headers
            .chunks(2)
            .map(|pair| {
                (
                    text(pair[0]),
                    pair.get(1).map(|value| text(value)).unwrap_or_default(),
                )
            })
            .collect(),
        body: body.to_vec(),
    };
    let response = with(|host| {
        host.responses
            .iter()
            .find(|(m, u, ..)| *m == fetched.method && *u == fetched.url)
            .map(|(.., status, body)| (*status, body.clone()))
    });
    let limit: Option<usize> = match core::str::from_utf8(limit).ok()? {
        "" => None,
        limit => Some(limit.parse().ok()?),
    };
    let Some((status, mut body)) = response else {
        panic!(
            "nothing arranged for {} {}; call berm_lang::test::respond first",
            fetched.method, fetched.url
        )
    };
    with(|host| host.fetched.push(fetched));
    if let Some(limit) = limit {
        body.truncate(limit);
    }
    Some((status, body))
}

/// Read a `(ptr, len)` pair the caller passed. Off-target these are real
/// addresses in this process.
fn bytes<'a>(ptr: u64, len: u64) -> &'a [u8] {
    unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) }
}

fn read(ptr: u64, len: u64) -> String {
    String::from_utf8_lossy(bytes(ptr, len)).into_owned()
}

#[cold]
fn no_host(number: u64) -> ! {
    panic!("host call {number:#x} has no stand-in; it needs a real host");
}
