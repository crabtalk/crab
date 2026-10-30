//! One HTTP request, to a host this service's manifest declares under
//! `berm.http.fetch`, or under `berm.http.proxy` to send it through the host's
//! proxy.
//!
//! A header value or the body may carry `{secret:NAME}` for a secret the
//! manifest declares. The host substitutes it, and this service never holds the value.

use crate::{
    abi::wire,
    host::{self, CallError},
    sys,
};
use alloc::{string::String, vec::Vec};
use serde::de::DeserializeOwned;

/// What the far end answered.
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// Whether the status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The value of the header `name`, ASCII case ignored.
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }

    /// The body read as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_slice(&self.body)
    }
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
    send(sys::HTTP_FETCH, method, url, headers, body)
}

/// [`fetch`], through the host's proxy. The host meters the bytes sent and
/// received against this service as `proxied`.
pub fn proxy(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, CallError> {
    send(sys::HTTP_PROXY, method, url, headers, body)
}

/// One request of a batch [`fetch_all`] sends.
pub struct Request<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
    /// Milliseconds to wait for an answer; `None` for the call's deadline.
    pub timeout: Option<u64>,
    /// The most bytes of the answer's body this service takes; the host cuts
    /// the rest before it reaches it. `None` for all of it.
    pub limit: Option<usize>,
}

/// Send `requests` at once, each to a host the manifest declares under
/// `berm.http.fetch` and given up at its own timeout or the call's deadline,
/// and answer each in its place: a [`Response`], or why nothing was.
pub fn fetch_all(requests: &[Request<'_>]) -> Result<Vec<Result<Response, CallError>>, CallError> {
    send_all(sys::HTTP_FETCH_ALL, requests)
}

/// [`fetch_all`], through the host's proxy, each request to a host the
/// manifest declares under `berm.http.proxy`. The host meters the bytes sent
/// and received against this service as `proxied`.
pub fn proxy_all(requests: &[Request<'_>]) -> Result<Vec<Result<Response, CallError>>, CallError> {
    send_all(sys::HTTP_PROXY_ALL, requests)
}

fn send_all(
    import: impl FnOnce(usize, usize) -> u64,
    requests: &[Request<'_>],
) -> Result<Vec<Result<Response, CallError>>, CallError> {
    let framed: Vec<Vec<u8>> = requests.iter().map(frame).collect();
    let fields: Vec<&[u8]> = framed.iter().map(Vec::as_slice).collect();
    let reply = host::call(import, &wire::request(&fields))?;
    let answers = wire::fields(&reply).map_err(|_| unreadable())?;
    if answers.len() != requests.len() {
        return Err(unreadable());
    }
    answers
        .into_iter()
        .map(|answer| {
            let fields = wire::fields(answer).map_err(|_| unreadable())?;
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
    import: impl FnOnce(usize, usize) -> u64,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, CallError> {
    let request = frame(&Request {
        method,
        url,
        headers,
        body,
        timeout: None,
        limit: None,
    });
    let reply = host::call(import, &request)?;
    response(&wire::fields(&reply).map_err(|_| unreadable())?)
}

/// `request` as [`abi::HTTP_FETCH`] frames it.
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

/// The value of the header `name` in `headers`, ASCII case ignored.
pub(crate) fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
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
