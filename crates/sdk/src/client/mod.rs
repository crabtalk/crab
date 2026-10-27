//! The HTTP client for a CrabTalk Cloud deployment, and for a local host.
//!
//! One [`Client`] holds the transport; each submodule hangs that surface's calls
//! off it. A local host answers Cloud's service paths for one account,
//! [`LOCAL`], and none of its account routes.

use crate::error::{ApiError, ErrorResponse};
use reqwest::{Method, RequestBuilder, Response, StatusCode, header::RETRY_AFTER};
use serde::{Deserialize, de::DeserializeOwned};
use std::{fmt, time::Duration};
mod app;
mod billing;
mod key;
mod meta;
mod org;
mod owner;
mod request;
mod service;
mod user;
pub use owner::{LOCAL, Owner};

/// Times a call is sent again after a `429`.
const RETRIES: usize = 3;

/// A connection to a CrabTalk Cloud deployment or a local host.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    pub base: String,
    pub token: Option<String>,
    /// The account Cloud deploys under; `None` for a local host.
    pub owner: Option<Owner>,
}

/// Why a call failed, and whether trying again can help.
///
/// Retrying past a definitive answer is how a diagnosable bug becomes a
/// "transient" that a restart appears to fix, so the three classes stay apart:
/// no answer, a refusal, and an answer neither side agrees about the shape of.
pub enum Error {
    /// No response: DNS, TLS, connect, or a body that would not read.
    Transport(reqwest::Error),
    /// The server answered and refused.
    Api {
        status: u16,
        /// The stable identifier cloud names a condition by, where it names
        /// one — `orgs_closed`. Branch on this rather
        /// than on `status`.
        code: Option<String>,
        message: String,
        /// When the condition self-resolves, in Unix seconds.
        reset_at: Option<i64>,
    },
    /// The call does not apply to this kind of host.
    Host(&'static str),
    /// The body did not match the type this build expects — a version skew
    /// between client and deployment. Carries serde's own words, which name
    /// the field; `.json()` would collapse them to "error decoding response
    /// body", which names neither the field nor the side that is stale.
    Decode(String),
}

/// A refusal arrives in one of two envelopes; serde picks by shape.
#[derive(Deserialize)]
#[serde(untagged)]
enum Refusal {
    Structured(ApiError),
    Plain(ErrorResponse),
}

impl Client {
    /// Point at a deployment — `http://localhost:5252` against a local server.
    /// A trailing slash is trimmed.
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into().trim_end_matches('/').to_string(),
            token: None,
            owner: None,
        }
    }

    /// The account the service calls act under on Cloud. Without one, they
    /// address a local host.
    pub fn owner(mut self, owner: Owner) -> Self {
        self.owner = Some(owner);
        self
    }

    /// The bearer token the authenticated routes need — a session token or an
    /// API key.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// The account the service paths are under: the context's on Cloud, and
    /// [`LOCAL`] on a local host.
    pub(crate) fn addressed(&self) -> Owner {
        self.owner
            .clone()
            .unwrap_or_else(|| Owner::User(LOCAL.to_owned()))
    }

    /// The account the client acts for, which only Cloud has.
    pub(crate) fn account(&self) -> Result<&Owner, Error> {
        self.owner
            .as_ref()
            .ok_or(Error::Host("only Cloud has accounts"))
    }

    /// A request against `path`, carrying the token when one is set.
    pub fn at(&self, method: Method, path: &str) -> RequestBuilder {
        let req = self.http.request(method, format!("{}{path}", self.base));
        match &self.token {
            Some(token) => req.bearer_auth(token),
            None => req,
        }
    }

    /// Send, and decode the body a success carries.
    pub async fn json<T: DeserializeOwned>(&self, req: RequestBuilder) -> Result<T, Error> {
        let body = self.ok(req).await?;
        serde_json::from_str(&body).map_err(|e| Error::Decode(e.to_string()))
    }

    /// Send `req`, sending it again each time a `429` names when to, after
    /// that wait, up to [`RETRIES`] times. A request whose body streams is
    /// sent once.
    async fn send(&self, req: RequestBuilder) -> Result<Response, Error> {
        let mut req = req;
        for _ in 0..RETRIES {
            let Some(again) = req.try_clone() else {
                break;
            };
            let res = req.send().await?;
            let wait = res
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok()?.parse::<u64>().ok());
            match (res.status(), wait) {
                (StatusCode::TOO_MANY_REQUESTS, Some(wait)) => {
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                    req = again;
                }
                _ => return Ok(res),
            }
        }
        Ok(req.send().await?)
    }

    /// Send, and read the body a success carries, turning anything outside 2xx
    /// into an [`Error::Api`].
    pub async fn ok(&self, req: RequestBuilder) -> Result<String, Error> {
        let res = self.send(req).await?;
        let status = res.status();
        let body = res.text().await?;
        if status.is_success() {
            return Ok(body);
        }
        Err(Error::refusal(status, body))
    }

    /// Send, and read the raw body a success carries.
    pub async fn bytes(&self, req: RequestBuilder) -> Result<Vec<u8>, Error> {
        let res = self.send(req).await?;
        let status = res.status();
        if status.is_success() {
            return Ok(res.bytes().await?.to_vec());
        }
        Err(Error::refusal(status, res.text().await?))
    }

    /// Send, and report only whether the deployment answered in the 2xx range.
    /// The health routes carry no body and say everything in the status.
    pub(crate) async fn probe(&self, req: RequestBuilder) -> Result<bool, Error> {
        Ok(self.send(req).await?.status().is_success())
    }
}

impl Error {
    /// What a non-2xx answer means: the structured envelope where the server
    /// sent one, the plain envelope where it sent that, and the body verbatim
    /// when it is neither.
    pub(crate) fn refusal(status: StatusCode, body: String) -> Self {
        let (code, message, reset_at) = match serde_json::from_str::<Refusal>(&body) {
            Ok(Refusal::Structured(e)) => (Some(e.error.code), e.error.message, e.error.reset_at),
            Ok(Refusal::Plain(e)) => (None, e.error, None),
            Err(_) => (None, body, None),
        };
        Self::Api {
            status: status.as_u16(),
            code,
            message,
            reset_at,
        }
    }

    /// Whether another attempt could answer differently. A refusal the server
    /// stood behind is not retried; nothing about waiting changes a `403`.
    pub fn retryable(&self) -> bool {
        match self {
            Self::Transport(_) => true,
            Self::Api { status, .. } => {
                *status == StatusCode::TOO_MANY_REQUESTS.as_u16() || *status >= 500
            }
            Self::Host(_) | Self::Decode(_) => false,
        }
    }
}

// The token is a credential, and a derived `Debug` would print it.
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("token", &self.token.as_ref().map(|_| "…"))
            .field("owner", &self.owner)
            .finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(_) => f.write_str("cannot reach the host"),
            Self::Api {
                status,
                code: Some(code),
                message,
                ..
            } => write!(f, "{status} {code}: {message}"),
            Self::Api {
                status, message, ..
            } => write!(f, "{status}: {message}"),
            Self::Host(e) => f.write_str(e),
            Self::Decode(e) => write!(f, "decoding the response: {e}"),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(e) => Some(e),
            Self::Api { .. } | Self::Host(_) | Self::Decode(_) => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Transport(e)
    }
}
