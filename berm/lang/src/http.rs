//! One HTTP request, to a host this program's manifest declares under
//! `berm.http.fetch`, or under `berm.http.proxy` to send it through the host's
//! proxy.
//!
//! A header value or the body may carry `{secret:NAME}` for a secret the
//! manifest declares. The host substitutes it, and this program never holds the value.

use crate::abi::{self, host, host::CallError, wire};
use alloc::{string::String, vec::Vec};

/// What the far end answered.
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Make one request and wait for its answer.
///
/// ```ignore
/// let page = berm_lang::http::fetch(
///     "GET",
///     "https://api.example.com/items",
///     &[("Authorization", "Bearer {secret:example}")],
///     b"",
/// )?;
/// ```
///
/// [`CallError::Refused`] means nothing was sent: a host or secret the
/// manifest does not declare, or a secret nobody granted. A status the far end
/// answered with, 4xx and 5xx included, is a [`Response`].
pub fn fetch(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, CallError> {
    send(abi::HOST_HTTP_FETCH, method, url, headers, body, None)
}

/// [`fetch`], given up when nothing answered within `timeout` milliseconds,
/// or at the call's deadline when that comes first.
pub fn fetch_within(
    timeout: u64,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, CallError> {
    send(
        abi::HOST_HTTP_FETCH,
        method,
        url,
        headers,
        body,
        Some(timeout),
    )
}

/// [`fetch`], through the host's proxy. The host meters the bytes sent and
/// received against this program as `proxied`.
pub fn proxy(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, CallError> {
    send(abi::HOST_HTTP_PROXY, method, url, headers, body, None)
}

/// One request of a batch [`fetch_all`] sends.
pub struct Request<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
    /// Milliseconds to wait for an answer; `None` for the call's deadline.
    pub timeout: Option<u64>,
    /// The most bytes of the answer's body this program takes; the host cuts
    /// the rest before it reaches it. `None` for all of it.
    pub limit: Option<usize>,
}

/// Send `requests` at once, each to a host the manifest declares under
/// `berm.http.fetch` and given up at its own timeout or the call's deadline,
/// and answer each in its place: a [`Response`], or why nothing was.
pub fn fetch_all(requests: &[Request<'_>]) -> Result<Vec<Result<Response, CallError>>, CallError> {
    let framed: Vec<Vec<u8>> = requests.iter().map(frame).collect();
    let fields: Vec<&[u8]> = framed.iter().map(Vec::as_slice).collect();
    let reply = host::call(abi::HOST_HTTP_FETCH_ALL, &wire::request(&fields))?;
    let answers = wire::fields(&reply).ok_or_else(unreadable)?;
    if answers.len() != requests.len() {
        return Err(unreadable());
    }
    answers
        .into_iter()
        .map(|answer| {
            let fields = wire::fields(answer).ok_or_else(unreadable)?;
            let (kind, rest) = fields.split_first().ok_or_else(unreadable)?;
            let why =
                || String::from_utf8_lossy(rest.first().copied().unwrap_or_default()).into_owned();
            Ok(match *kind {
                b"ok" => response(rest),
                b"refused" => Err(CallError::Refused(why())),
                b"failed" => Err(CallError::Failed(why())),
                _ => return Err(unreadable()),
            })
        })
        .collect()
}

fn send(
    number: u64,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    timeout: Option<u64>,
) -> Result<Response, CallError> {
    let request = frame(&Request {
        method,
        url,
        headers,
        body,
        timeout,
        limit: None,
    });
    let reply = host::call(number, &request)?;
    response(&wire::fields(&reply).ok_or_else(unreadable)?)
}

/// `request` as [`abi::HOST_HTTP_FETCH`] frames it.
fn frame(request: &Request<'_>) -> Vec<u8> {
    let timeout = request
        .timeout
        .map(|timeout| alloc::format!("{timeout}"))
        .unwrap_or_default();
    let limit = request
        .limit
        .map(|limit| alloc::format!("{limit}"))
        .unwrap_or_default();
    let mut framed = wire::request(&[
        request.method.as_bytes(),
        request.url.as_bytes(),
        request.body,
        timeout.as_bytes(),
        limit.as_bytes(),
    ]);
    for (name, value) in request.headers {
        wire::field(&mut framed, name.as_bytes());
        wire::field(&mut framed, value.as_bytes());
    }
    framed
}

/// The [`Response`] a reply's `fields` hold.
fn response(fields: &[&[u8]]) -> Result<Response, CallError> {
    let [status, body, rest @ ..] = fields else {
        return Err(unreadable());
    };
    if rest.len() % 2 != 0 {
        return Err(unreadable());
    }
    let status = core::str::from_utf8(status)
        .ok()
        .and_then(|status| status.parse().ok())
        .ok_or_else(unreadable)?;
    let headers = rest
        .chunks(2)
        .map(|pair| {
            (
                String::from_utf8_lossy(pair[0]).into_owned(),
                String::from_utf8_lossy(pair[1]).into_owned(),
            )
        })
        .collect();
    Ok(Response {
        status,
        headers,
        body: body.to_vec(),
    })
}

/// What a reply this guest cannot read fails with.
fn unreadable() -> CallError {
    CallError::Failed(String::from("host framed a reply this guest cannot read"))
}

/// `fields` as `application/x-www-form-urlencoded`.
pub fn form(fields: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (name, value) in fields {
        if !out.is_empty() {
            out.push('&');
        }
        out.push_str(&encode(name));
        out.push('=');
        out.push_str(&encode(value));
    }
    out
}

/// `text` percent-encoded, leaving only unreserved characters.
pub fn encode(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 15) as usize] as char);
            }
        }
    }
    out
}
