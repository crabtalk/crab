//! The edges a daemon gives its [`Host`](crate::Host): what differs between
//! running services on a laptop and on a fleet's node.

use crate::Layers;
use anyhow::{Result, bail};
use berm::{
    Callsite, Enter, Meter, Refused, clock,
    syscall::http::{Request, Response},
};
use berm_api::host::{Link, Reply, Used};
use std::future::Future;

/// One daemon's edges, as types.
pub trait Env: Clone + Send + Sync + 'static {
    /// Sends `berm.http.fetch`, and `berm.http.proxy` through a proxy. Follows
    /// no redirects; the host bounds each request by its call's deadline.
    type Fetch: tower::Service<Request, Response = Response, Error = anyhow::Error, Future: Send + 'static>
        + Clone
        + Send
        + Sync
        + 'static;
    type Images: Images;
    type Peers: Peers;
    type Secrets: Secrets;
    type Observe: Observe;
    type Clock: Clock;
}

/// One daemon's edges, as values.
pub struct Edges<E: Env> {
    pub fetch: E::Fetch,
    /// What `berm.http.proxy` is sent through; without one, a manifest that
    /// declares it is refused at deploy.
    pub proxy: Option<E::Fetch>,
    pub images: E::Images,
    pub peers: E::Peers,
    pub secrets: E::Secrets,
    /// Put around every call from outside, outermost first. A refusal one
    /// answers with a `4xx` is counted as `refused`.
    pub layers: Layers,
    /// Additional invocation scope, opened before the host database scope.
    /// The host retains journal ownership. A returned `Denied` keeps its HTTP status.
    pub enter: Option<Enter>,
    pub observe: E::Observe,
    /// What `berm.now` answers, when a wake is due, and the day a use or a
    /// meter is counted on.
    pub clock: E::Clock,
}

/// A service's bytes for a deploy.
pub trait Images: Send + Sync + 'static {
    /// The image `digest` names, published as `reference`.
    fn pull(&self, reference: &str, digest: &str) -> impl Future<Output = Result<Vec<u8>>> + Send;
}

/// The hosts running the services this one does not, each named by the peer a
/// [`Route`](berm_api::host::Route) gives.
pub trait Peers: Send + Sync + 'static {
    /// Call `tool` of `service` on `peer` as `link`. A refusal there is a
    /// [`berm::Refused`] here.
    fn link(
        &self,
        peer: &str,
        service: &str,
        tool: &str,
        args: Vec<u8>,
        link: Link,
    ) -> impl Future<Output = Result<Reply>> + Send;

    /// Call a qualified deployment through the account authorization boundary.
    fn call(
        &self,
        service: &str,
        _tool: &str,
        _args: Vec<u8>,
        _link: Link,
    ) -> impl Future<Output = Result<Reply>> + Send {
        async move { Err(Refused(format!("this host has no account gateway for {service:?}")).into()) }
    }

    /// Hand `peer` the uses counted here of services it runs.
    fn ship(&self, peer: &str, used: &[Used]) -> impl Future<Output = Result<()>> + Send;
}

/// No peers: every service runs here, and no route names one.
pub struct Alone;

impl Peers for Alone {
    async fn link(&self, peer: &str, _: &str, _: &str, _: Vec<u8>, _: Link) -> Result<Reply> {
        bail!("no peer {peer:?}")
    }

    async fn ship(&self, peer: &str, _: &[Used]) -> Result<()> {
        bail!("no peer {peer:?}")
    }
}

/// What `{secret:NAME}` in a fetch header stands for.
pub trait Secrets: Send + Sync + 'static {
    /// The value of `secret` granted to `service` for `host`, if any.
    fn grant(
        &self,
        service: &str,
        secret: &str,
        host: &str,
    ) -> impl Future<Output = Result<Option<String>>> + Send;

    /// Owner of a resource granted to this deployment and destination; no value.
    fn resource_owner(
        &self,
        _service: &str,
        _id: &str,
        _host: &str,
    ) -> impl Future<Output = Result<Option<String>>> + Send {
        async { Ok(None) }
    }

    /// The names of the secrets provisioned for `service`.
    fn names(&self, service: &str) -> impl Future<Output = Result<Vec<String>>> + Send;
}

/// Told of each invocation after it ran, trapped or not. Cannot change how it ended.
pub trait Observe: Send + Sync + 'static {
    /// `call` is the recorded run on this host, or `None` for a peer link.
    /// `used` excludes nested calls; `at.spent()` includes their CPU time.
    fn invoked(&self, at: &Callsite, call: Option<u64>, used: &Meter);
}

impl Observe for () {
    fn invoked(&self, _: &Callsite, _: Option<u64>, _: &Meter) {}
}

impl<O: Observe> Observe for std::sync::Arc<O> {
    fn invoked(&self, at: &Callsite, call: Option<u64>, used: &Meter) {
        (**self).invoked(at, call, used);
    }
}

/// What time it is.
pub trait Clock: Send + Sync + 'static {
    /// Milliseconds since the Unix epoch. Called inside a syscall, where an
    /// unwind aborts the process.
    fn now(&self) -> u64;
}

/// The system's clock.
pub struct System;

impl Clock for System {
    fn now(&self) -> u64 {
        clock::now()
    }
}
