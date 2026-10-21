//! `berm.http.fetch` and `berm.http.proxy` — one HTTP request, to a host the
//! program's manifest declares under the syscall it makes.
//!
//! berm checks the request against the asking program's [`crate::Deps`] and
//! substitutes every `{secret:NAME}` in a header value and in the body. Resolving a secret and
//! sending the request are the host's, and arrive as the two closures below.

use crate::{Callsite, Reach, Refused, Syscall, abi, wire};
use anyhow::{Result, bail};
use futures_util::future::join_all;
use std::{future::Future, sync::Arc, time::Duration};

/// What opens a secret's name in a header value.
const SECRET: &str = "{secret:";

/// One request, as the host is handed it: allowed by the manifest, and every
/// secret already substituted.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub url: String,
    /// The host `url` names, lowercased, without port or credentials.
    pub host: String,
    /// How the asking program's manifest declares `host`.
    pub reach: Reach,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// How long the asking program waits for an answer, within its call's
    /// deadline; `None` for the deadline alone.
    pub timeout: Option<Duration>,
}

impl Request {
    /// Its bytes on the wire as metered: the URL, header names and values, and
    /// body.
    pub fn size(&self) -> usize {
        self.url.len() + self.body.len() + headers(&self.headers)
    }
}

/// What the host got back.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// Its bytes as metered: header names and values, and body.
    pub fn size(&self) -> usize {
        self.body.len() + headers(&self.headers)
    }
}

/// Serve `berm.http.fetch`.
///
/// `secret` is handed the callsite, a secret's name and the request's host,
/// and answers the value granted for that triple or `None`. `send` performs
/// the request; it is not handed one the manifest does not allow. A guest
/// suspended on `send` cannot notice its deadline, so `send` bounds itself by
/// [`Callsite::deadline`] and does not follow redirects.
pub fn fetch<S, Sent, R, Found>(send: S, secret: R) -> Syscall
where
    S: Fn(Callsite, Request) -> Sent + Send + Sync + 'static,
    Sent: Future<Output = Result<Response>> + Send + 'static,
    R: Fn(Callsite, String, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<String>>> + Send + 'static,
{
    serve(abi::HTTP_FETCH, send, secret)
}

/// Serve `berm.http.proxy`: [`fetch`], to the hosts the manifest declares under
/// it, where `send` goes through the host's proxy.
pub fn proxy<S, Sent, R, Found>(send: S, secret: R) -> Syscall
where
    S: Fn(Callsite, Request) -> Sent + Send + Sync + 'static,
    Sent: Future<Output = Result<Response>> + Send + 'static,
    R: Fn(Callsite, String, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<String>>> + Send + 'static,
{
    serve(abi::HTTP_PROXY, send, secret)
}

/// Serve `berm.http.fetch_all`: a batch of [`fetch`] requests, each to a
/// host the manifest declares under `berm.http.fetch`, sent through `send` at
/// once and answered in order, a refusal or failure answering its own place.
pub fn fetch_all<S, Sent, R, Found>(send: S, secret: R) -> Syscall
where
    S: Fn(Callsite, Request) -> Sent + Send + Sync + 'static,
    Sent: Future<Output = Result<Response>> + Send + 'static,
    R: Fn(Callsite, String, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<String>>> + Send + 'static,
{
    let (send, secret) = (Arc::new(send), Arc::new(secret));
    Syscall {
        name: abi::HTTP_FETCH_ALL.to_owned(),
        call: Arc::new(move |at: Callsite, batch: Vec<u8>| {
            let (send, secret) = (send.clone(), secret.clone());
            Box::pin(async move {
                let requests: Vec<Vec<u8>> = wire::fields(&batch)?
                    .into_iter()
                    .map(<[u8]>::to_vec)
                    .collect();
                let answers = join_all(
                    requests
                        .iter()
                        .map(|request| answer(&at, abi::HTTP_FETCH, request, &*send, &*secret)),
                )
                .await;
                let framed: Vec<Vec<u8>> = answers
                    .iter()
                    .map(|answered| match answered {
                        Ok(response) => {
                            let mut fields: Vec<&[u8]> = vec![b"ok"];
                            let written = write(response);
                            let inner = wire::fields(&written).unwrap_or_default();
                            fields.extend(inner);
                            wire::frame(&fields)
                        }
                        Err(error) => {
                            let kind: &[u8] = match error.downcast_ref::<Refused>() {
                                Some(_) => b"refused",
                                None => b"failed",
                            };
                            wire::frame(&[kind, format!("{error:#}").as_bytes()])
                        }
                    })
                    .collect();
                let fields: Vec<&[u8]> = framed.iter().map(Vec::as_slice).collect();
                Ok(wire::frame(&fields))
            })
        }),
    }
}

fn serve<S, Sent, R, Found>(name: &'static str, send: S, secret: R) -> Syscall
where
    S: Fn(Callsite, Request) -> Sent + Send + Sync + 'static,
    Sent: Future<Output = Result<Response>> + Send + 'static,
    R: Fn(Callsite, String, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<String>>> + Send + 'static,
{
    let (send, secret) = (Arc::new(send), Arc::new(secret));
    Syscall {
        name: name.to_owned(),
        call: Arc::new(move |at: Callsite, request: Vec<u8>| {
            let (send, secret) = (send.clone(), secret.clone());
            Box::pin(async move {
                let response = answer(&at, name, &request, &*send, &*secret).await?;
                Ok(write(&response))
            })
        }),
    }
}

/// Send `request`, read off the wire and allowed under `syscall`, its secrets
/// substituted, through `send`.
async fn answer<S, Sent, R, Found>(
    at: &Callsite,
    syscall: &str,
    request: &[u8],
    send: &S,
    secret: &R,
) -> Result<Response>
where
    S: Fn(Callsite, Request) -> Sent,
    Sent: Future<Output = Result<Response>>,
    R: Fn(Callsite, String, String) -> Found,
    Found: Future<Output = Result<Option<String>>>,
{
    let (mut request, limit) = read(at, syscall, request)?;
    for (_, value) in &mut request.headers {
        let found = substitute(at, secret, &request.host, value.as_bytes()).await?;
        if let Some(found) = found {
            *value = String::from_utf8(found)?;
        }
    }
    if let Some(found) = substitute(at, secret, &request.host, &request.body).await? {
        request.body = found;
    }
    let mut response = send(at.clone(), request).await?;
    if let Some(limit) = limit {
        response.body.truncate(limit);
    }
    Ok(response)
}

/// `text` with every `{secret:NAME}` replaced by what `secret` grants the
/// asking program for `host`, or `None` when it names no secret.
async fn substitute<R, Found>(
    at: &Callsite,
    secret: &R,
    host: &str,
    text: &[u8],
) -> Result<Option<Vec<u8>>>
where
    R: Fn(Callsite, String, String) -> Found,
    Found: Future<Output = Result<Option<String>>>,
{
    let opens = |rest: &[u8]| {
        rest.windows(SECRET.len())
            .position(|w| w == SECRET.as_bytes())
    };
    if opens(text).is_none() {
        return Ok(None);
    }
    let mut substituted = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = opens(rest) {
        let after = &rest[start + SECRET.len()..];
        let Some(end) = after.iter().position(|&byte| byte == b'}') else {
            bail!("{SECRET} is opened and never closed");
        };
        let name = std::str::from_utf8(&after[..end])?;
        if !at.deps.secrets.iter().any(|declared| declared == name) {
            return Err(Refused(format!(
                "{} does not declare the secret {name:?}",
                at.program
            ))
            .into());
        }
        let Some(found) = secret(at.clone(), name.to_owned(), host.to_owned()).await? else {
            return Err(Refused(format!(
                "no secret {name:?} is granted to {} for {host}",
                at.program
            ))
            .into());
        };
        substituted.extend_from_slice(&rest[..start]);
        substituted.extend_from_slice(found.as_bytes());
        rest = &after[end + 1..];
    }
    substituted.extend_from_slice(rest);
    Ok(Some(substituted))
}

/// The bytes of `headers`' names and values.
fn headers(headers: &[(String, String)]) -> usize {
    headers
        .iter()
        .map(|(name, value)| name.len() + value.len())
        .sum()
}

/// Refuse a host the asking program's manifest does not declare under
/// `syscall`.
fn permitted(at: &Callsite, syscall: &str, host: &str) -> Result<Reach> {
    at.deps.reach(syscall, host).ok_or_else(|| {
        Refused(format!(
            "{} does not declare {host} under {syscall}",
            at.program
        ))
        .into()
    })
}

/// `request` off the wire, refused unless the asking program declares its host
/// under `syscall`, with the most body bytes the program takes of its answer.
fn read(at: &Callsite, syscall: &str, request: &[u8]) -> Result<(Request, Option<usize>)> {
    let fields = wire::fields(request)?;
    let method = wire::text(&fields, 0, "method")?.to_owned();
    let url = wire::text(&fields, 1, "url")?.to_owned();
    let Some(body) = fields.get(2) else {
        bail!("request has no body");
    };
    let timeout = match wire::text(&fields, 3, "timeout")? {
        "" => None,
        millis => Some(Duration::from_millis(millis.parse().map_err(|_| {
            anyhow::anyhow!("timeout {millis:?} is not a count of milliseconds")
        })?)),
    };
    let limit = match wire::text(&fields, 4, "limit")? {
        "" => None,
        bytes => Some(
            bytes
                .parse()
                .map_err(|_| anyhow::anyhow!("limit {bytes:?} is not a count of bytes"))?,
        ),
    };
    let trailing = &fields[5..];
    if trailing.len() % 2 != 0 {
        bail!("a header has a name and no value");
    }
    let mut headers = Vec::with_capacity(trailing.len() / 2);
    for pair in trailing.chunks(2) {
        headers.push((
            std::str::from_utf8(pair[0])?.to_owned(),
            std::str::from_utf8(pair[1])?.to_owned(),
        ));
    }
    let Some(host) = host(&url) else {
        bail!("{url} is not an http or https URL with a host");
    };
    let host = host.to_ascii_lowercase();
    let reach = permitted(at, syscall, &host)?;
    Ok((
        Request {
            method,
            host,
            reach,
            url,
            headers,
            body: body.to_vec(),
            timeout,
        },
        limit,
    ))
}

fn write(response: &Response) -> Vec<u8> {
    let status = response.status.to_string();
    let mut fields: Vec<&[u8]> = vec![status.as_bytes(), &response.body];
    for (name, value) in &response.headers {
        fields.push(name.as_bytes());
        fields.push(value.as_bytes());
    }
    wire::frame(&fields)
}

/// The host an `http` or `https` URL names, without port or credentials.
fn host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    let host = match authority.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next()?,
        None => authority.split(':').next()?,
    };
    (!host.is_empty()).then_some(host)
}
