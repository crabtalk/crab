//! Calls from outside: what the layers a daemon puts around its host see, and
//! the host's own service at the bottom of them.

use crate::{
    Denied, deny,
    env::Env,
    host::{self, Host, Inner},
    record::{calls, usage},
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use berm::{
    Meter,
    tape::{Recorder, Taping},
};
use berm_api::{
    Output,
    host::{self as wire, Caller, Kind, Outcome, Source},
};
use std::{
    future::Future,
    pin::Pin,
    sync::Weak,
    task::{Context, Poll},
};
use tower::{
    Service, ServiceExt,
    util::{BoxCloneSyncService, BoxCloneSyncServiceLayer},
};

/// A call from outside of a public tool that takes calls from accounts, as
/// the layers around a host see it.
#[derive(Debug, Clone)]
pub struct Call {
    /// The service, as its daemon names it.
    pub service: String,
    pub tool: String,
    /// Whether the tool is a query.
    pub query: bool,
    pub args: Vec<u8>,
    pub caller: Option<Caller>,
    /// The outside client's per-day key, as a layer sets it; the chain reads it
    /// through `berm.client`.
    pub client: Option<String>,
    /// The request's, as the daemon received it.
    pub headers: HeaderMap,
}

/// How a call from outside ended.
#[derive(Debug)]
pub struct Called {
    /// What the answer carries: its meter, and the tool's deprecation.
    pub headers: HeaderMap,
    pub output: Output,
    /// Its id among its service's calls.
    pub id: u64,
}

/// A service taking calls from outside.
pub type Outside = BoxCloneSyncService<Call, Called, Denied>;

/// What a daemon puts around [`Outside`].
pub type Layers = BoxCloneSyncServiceLayer<Outside, Call, Called, Denied>;

impl<E: Env> Host<E> {
    /// Run `tool` on `service` from outside, as `caller` when there is one,
    /// counted as its use. `query` holds it to a tool the IDL marks `query`.
    /// Answers with the deprecation headers the tool's answer carries.
    pub async fn call(
        &self,
        service: &str,
        caller: Option<&Caller>,
        tool: &str,
        args: Vec<u8>,
        query: bool,
    ) -> Result<(HeaderMap, Output), Denied> {
        let called = self
            .outside(
                service,
                tool,
                query,
                args,
                caller.cloned(),
                HeaderMap::new(),
            )
            .await?;
        Ok((called.headers, called.output))
    }

    /// A call from outside, through the daemon's layers. `query` holds it to a
    /// tool the IDL marks `query`; `headers` are the request's.
    pub async fn outside(
        &self,
        service: &str,
        tool: &str,
        query: bool,
        args: Vec<u8>,
        caller: Option<Caller>,
        headers: HeaderMap,
    ) -> Result<Called, Denied> {
        let deployed = self.deployed(service)?;
        let Some(spec) = deployed
            .manifest
            .tools
            .iter()
            .find(|spec| spec.name == tool && spec.public)
        else {
            return Err(deny(
                StatusCode::NOT_FOUND,
                format!("the service has no public tool named {tool:?}"),
            ));
        };
        if query && !spec.query {
            return Err(deny(
                StatusCode::METHOD_NOT_ALLOWED,
                format!("{tool} is not a query; POST it"),
            ));
        }
        if self.stops.stopped(service) {
            return Err(deny(
                StatusCode::SERVICE_UNAVAILABLE,
                format!("{service} is stopped"),
            ));
        }
        let refused = |denied: Denied, caller: Option<&Caller>| {
            let (source, who) = counted(caller);
            self.usage
                .record(service, Kind::Tool, tool, source, who, Outcome::Refused);
            self.calls.record(
                service,
                self.calls.start(),
                tool,
                source,
                who,
                Outcome::Refused,
                None,
                None,
                None,
            );
            Err(denied)
        };
        if !deployed.reached_by_accounts(tool) {
            return refused(
                deny(
                    StatusCode::FORBIDDEN,
                    format!("{tool} takes calls from services only"),
                ),
                caller.as_ref(),
            );
        }
        if !spec.anonymous && caller.as_ref().is_none_or(Caller::anonymous) {
            return refused(
                deny(
                    StatusCode::UNAUTHORIZED,
                    format!("{tool} takes no anonymous calls; call it as a caller"),
                ),
                caller.as_ref(),
            );
        }
        let call = Call {
            service: service.to_owned(),
            tool: tool.to_owned(),
            query: spec.query,
            args,
            caller: caller.clone(),
            client: None,
            headers,
        };
        match self.outside.clone().oneshot(call).await {
            Err(denied) if denied.status.is_client_error() => refused(denied, caller.as_ref()),
            answered => answered,
        }
    }

    /// The bottom of the layers: a call the layers let through, run.
    async fn run(&self, call: Call) -> Result<Called, Denied> {
        let Call {
            service,
            tool,
            args,
            caller,
            client,
            ..
        } = call;
        let deployed = self
            .get(&service)
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such service"))?;
        let deprecated = deployed
            .manifest
            .tools
            .iter()
            .any(|spec| spec.name == tool && spec.deprecated);
        let mut headers = HeaderMap::new();
        let (scope, name) = host::split(&service);
        if deprecated
            && let Some((since, sunset)) =
                usage::deprecation(&self.store, scope, name, &tool).await?
        {
            headers.insert(
                HeaderName::from_static("deprecation"),
                HeaderValue::from_str(&format!("@{since}")).expect("a number is a header value"),
            );
            if let Some(at) = sunset.and_then(|at| chrono::DateTime::from_timestamp(at, 0)) {
                headers.insert(
                    HeaderName::from_static("sunset"),
                    HeaderValue::from_str(&at.format("%a, %d %b %Y %H:%M:%S GMT").to_string())
                        .expect("a date is a header value"),
                );
            }
        }
        if self.pools.pool(scope).full() {
            return Err(deny(
                StatusCode::TOO_MANY_REQUESTS,
                "the account holds as much memory as it may; retry",
            ));
        }
        if self.reserve.full() {
            return Err(deny(
                StatusCode::SERVICE_UNAVAILABLE,
                "the host holds as much memory as it may; retry",
            ));
        }
        let Ok(_slot) = self.gate.try_acquire() else {
            return Err(deny(
                StatusCode::SERVICE_UNAVAILABLE,
                "every call slot is taken; retry",
            ));
        };
        let started = self.calls.start();
        let recorder = Recorder::default();
        let kept = args.clone();
        let input = |args| calls::ran_with(&tool, args, caller.clone(), &deployed.digest);
        let (source, who) = counted(caller.as_ref());
        let invoked = self
            .invoke(
                &deployed,
                started.id,
                caller
                    .as_ref()
                    .filter(|caller| !caller.anonymous())
                    .map(|caller| caller.id.as_str()),
                caller
                    .as_ref()
                    .and_then(|caller| caller.user.as_ref())
                    .map(host::user),
                client.map(Into::into),
                &tool,
                args,
                Taping::Record(recorder.clone()),
            )
            .await;
        let mut invoked = match invoked {
            Ok(invoked) => invoked,
            Err(e) => {
                self.usage
                    .record(&service, Kind::Tool, &tool, source, who, Outcome::Failed);
                self.calls.record(
                    &service,
                    started,
                    &tool,
                    source,
                    who,
                    Outcome::Failed,
                    None,
                    Some(input(kept)),
                    Some(&recorder.tape()),
                );
                return Err(deny(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")));
            }
        };
        let (Output::Done(body) | Output::Failed(body)) = &invoked.result;
        let answer = Meter {
            sent: body.len() as u64,
            ..Meter::default()
        };
        self.meters.record(&service, &tool, answer);
        invoked.meter += answer;
        let id = started.id;
        let outcome = Outcome::of(&invoked.result, invoked.denied);
        self.usage
            .record(&service, Kind::Tool, &tool, source, who, outcome);
        let failed = outcome == Outcome::Failed;
        self.calls.record(
            &service,
            started,
            &tool,
            source,
            who,
            outcome,
            Some(&invoked),
            failed.then(|| input(kept)),
            failed.then(|| recorder.tape()).as_ref(),
        );
        headers.insert(
            HeaderName::from_static(wire::METER),
            HeaderValue::from_str(&serde_json::to_string(&invoked.meter).expect("a meter is JSON"))
                .expect("JSON of numbers is a header value"),
        );
        Ok(Called {
            headers,
            output: invoked.result,
            id,
        })
    }
}

/// The host's own service, at the bottom of the daemon's layers.
pub(crate) struct Bottom<E: Env>(pub(crate) Weak<Inner<E>>);

impl<E: Env> Clone for Bottom<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: Env> Service<Call> for Bottom<E> {
    type Response = Called;
    type Error = Denied;
    type Future = Pin<Box<dyn Future<Output = Result<Called, Denied>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Denied>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, call: Call) -> Self::Future {
        let host = host::alive(&self.0).map(Host);
        Box::pin(async move { host?.run(call).await })
    }
}

/// How a call from outside is counted: by its app, by its account, or as
/// anonymous without a caller.
fn counted(caller: Option<&Caller>) -> (Source, &str) {
    match caller {
        Some(Caller { app: Some(app), .. }) => (Source::App, app),
        Some(caller) => (Source::Account, &caller.id),
        None => (Source::Anonymous, ""),
    }
}
