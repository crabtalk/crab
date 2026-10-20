//! `berm.http.fetch` and `berm.http.proxy`, sent from this process.

use anyhow::{Context, Result, bail};
use berm::{
    Reach,
    syscall::http::{Request, Response},
};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr},
    pin::Pin,
    task::{self, Poll},
};

/// The largest body a response may carry. The guest reads it into its own
/// heap.
const MAX_BODY: usize = 16 << 20;

/// A client that answers a redirect to the guest as one.
///
/// A host the manifest reaches only through a pattern is refused when it is,
/// or resolves only to, an address that is not public. Through a proxy, the
/// proxy resolves the name, and only an address written in the URL is checked.
#[derive(Clone)]
pub struct Fetch {
    named: reqwest::Client,
    pattern: reqwest::Client,
}

impl Fetch {
    /// Through `proxy` when given, with no idle connection kept, so a rotating
    /// proxy gives each request a fresh exit.
    pub fn new(proxy: Option<&str>) -> Result<Self> {
        let client = |resolve: Option<Public>| -> Result<reqwest::Client> {
            let mut client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
            if let Some(proxy) = proxy {
                client = client
                    .proxy(reqwest::Proxy::all(proxy).context("the proxy URL is not one")?)
                    .pool_max_idle_per_host(0);
            }
            if let (None, Some(resolve)) = (proxy, resolve) {
                client = client.dns_resolver(resolve);
            }
            client.build().context("failed to build the HTTP client")
        };
        Ok(Self {
            named: client(None)?,
            pattern: client(Some(Public))?,
        })
    }
}

/// Resolves a name to its public addresses, and fails when it has none.
struct Public;

impl Resolve for Public {
    fn resolve(&self, name: Name) -> Resolving {
        let name = name.as_str().to_owned();
        Box::pin(async move {
            let found: Vec<_> = tokio::net::lookup_host((name.as_str(), 0))
                .await?
                .filter(|addr| public(addr.ip()))
                .collect();
            if found.is_empty() {
                return Err(format!("{name} resolves to no public address").into());
            }
            Ok(Box::new(found.into_iter()) as Addrs)
        })
    }
}

/// Whether `ip` is reachable on the public internet.
fn public(ip: IpAddr) -> bool {
    let v4 = |ip: Ipv4Addr| {
        let [a, b, ..] = ip.octets();
        !(ip.is_loopback()
            || ip.is_private()
            || ip.is_link_local()
            || ip.is_unspecified()
            || ip.is_broadcast()
            || ip.is_documentation()
            || ip.is_multicast()
            || a == 0
            || (a == 100 && (64..128).contains(&b)))
    };
    match ip {
        IpAddr::V4(ip) => v4(ip),
        IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
            Some(ip) => v4(ip),
            None => {
                !(ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_multicast()
                    || ip.is_unique_local()
                    || ip.is_unicast_link_local())
            }
        },
    }
}

impl tower::Service<Request> for Fetch {
    type Response = Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response>> + Send>>;

    fn poll_ready(&mut self, _: &mut task::Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let client = match request.reach {
            Reach::Named => self.named.clone(),
            Reach::Pattern => self.pattern.clone(),
        };
        Box::pin(async move {
            if request.reach == Reach::Pattern
                && let Ok(ip) = request.host.trim_matches(['[', ']']).parse::<IpAddr>()
                && !public(ip)
            {
                bail!("{} is not a public address", request.host);
            }
            let method = reqwest::Method::from_bytes(request.method.as_bytes())
                .with_context(|| format!("{:?} is not an HTTP method", request.method))?;
            let mut sending = client.request(method, &request.url).body(request.body);
            for (name, value) in &request.headers {
                sending = sending.header(name, value);
            }
            let mut response = sending.send().await?;
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                if body.len() + chunk.len() > MAX_BODY {
                    bail!("{} answered with more than {MAX_BODY} bytes", request.host);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(Response {
                status,
                headers,
                body,
            })
        })
    }
}
