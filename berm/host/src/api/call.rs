//! What a daemon's routes ask of a host: calls from peers, and reading and
//! setting what each service is.

use crate::{
    Denied, Stored, deny,
    env::{Env, Images, Secrets},
    host::{self, Host},
    record::{calls, logs, meter, records, usage},
    state::{service, vars},
    wake::{cron, table},
};
use axum::http::StatusCode;
use berm::{
    Budget, Meters, Refused, Service,
    tape::{Recorder, Taping},
};
use berm_api::{
    Deps, Listing, Logged, Meter, Output, Setting, Terms, ToolTerms,
    host::{self as wire, Kind, Link, Metered, Outcome, Reply, Route, Source, Usage, Used},
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};

/// The most logs or calls one page answers.
const MAX_PAGE: i64 = 1000;

impl<E: Env> Host<E> {
    /// `service`'s IDL.
    pub fn idl(&self, service: &str) -> Result<berm_api::Service, Denied> {
        let deployed = self.deployed(service)?;
        Ok(deployed.idl(host::split(service).1))
    }

    /// `service` as anyone it is visible to sees it.
    pub async fn listing(&self, service: &str) -> Result<Listing, Denied> {
        let idl = self.idl(service)?;
        let (scope, name) = host::split(service);
        let served = meter::served(
            &self.store,
            scope,
            Some(name),
            self.today(),
            self.meters.days_of(scope, Some(name), i32::MIN),
        )
        .await?
        .remove(name)
        .unwrap_or_default();
        let row = service::row(&self.store, scope, name).await?;
        Ok(Listing {
            service: idl,
            served,
            stopped: self.stops.stopped(service),
            reference: row.as_ref().and_then(service::Row::registry),
            project: row.and_then(|row| row.project),
        })
    }

    /// `service`'s runtime, for its owners, with what it declares that nothing
    /// answers to: services neither deployed here nor routed to a peer, and
    /// secrets not granted it. Secrets that cannot be read grant nothing.
    pub async fn runtime(&self, service: &str) -> Result<berm_api::Runtime, Denied> {
        let deployed = self.deployed(service)?;
        let scope = host::split(service).0;
        let deps = &deployed.manifest.deps;
        let granted = self.secrets.names(service).await.unwrap_or_default();
        let project = service::row(&self.store, scope, host::split(service).1)
            .await?
            .and_then(|row| row.project);
        Ok(berm_api::Runtime {
            unresolved: Deps {
                calls: deps
                    .calls
                    .iter()
                    .filter(|name| {
                        let key = host::join(scope, name);
                        self.get(&key).is_none() && self.routes.peer(&key).is_none()
                    })
                    .cloned()
                    .collect(),
                syscalls: Default::default(),
                credentials: Default::default(),
                vars: Default::default(),
                secrets: deps
                    .secrets
                    .iter()
                    .filter(|name| !granted.contains(name))
                    .cloned()
                    .collect(),
            },
            project,
            ..deployed.runtime()
        })
    }

    /// Every service in `scope` this host runs, as anyone it is visible to
    /// sees it.
    pub async fn listings(&self, scope: &str) -> Result<Vec<Listing>, Denied> {
        let mut served = meter::served(
            &self.store,
            scope,
            None,
            self.today(),
            self.meters.days_of(scope, None, i32::MIN),
        )
        .await?;
        let mut rows = service::scoped(&self.store, scope).await?;
        Ok(self
            .list(scope)
            .into_iter()
            .map(|(name, deployed)| {
                let row = rows.remove(&name);
                Listing {
                    service: deployed.idl(&name),
                    served: served.remove(&name).unwrap_or_default(),
                    stopped: self.stops.stopped(&deployed.name),
                    reference: row.as_ref().and_then(service::Row::registry),
                    project: row.and_then(|row| row.project),
                }
            })
            .collect())
    }

    /// `service`'s terms: each public tool's budget, what its owner set or
    /// else this host's, its memory this host's most where the owner set none,
    /// and this host's deadline.
    pub fn terms(&self, service: &str) -> Result<Terms, Denied> {
        let deployed = self.deployed(service)?;
        let tools = deployed
            .manifest
            .tools
            .iter()
            .filter(|tool| tool.public)
            .map(|tool| {
                let mut budget = self
                    .terms
                    .get(service, &tool.name)
                    .budget
                    .unwrap_or(self.budget);
                if budget.memory == 0 {
                    budget.memory = self.budget.memory;
                }
                (tool.name.clone(), ToolTerms { budget })
            })
            .collect();
        Ok(Terms {
            tools,
            deadline: self.deadline.as_millis() as u64,
        })
    }

    /// Set what one invocation of `tool` of `service` may use, from its next
    /// call. Refused past this host's budget.
    pub async fn set_budget(
        &self,
        service: &str,
        tool: &str,
        budget: Budget,
    ) -> Result<(), Denied> {
        self.exported(service, tool)?;
        if budget.cpu == 0
            || budget.rows == 0
            || budget.cpu > self.budget.cpu
            || budget.rows > self.budget.rows
            || budget.memory > self.budget.memory
        {
            return Err(deny(
                StatusCode::BAD_REQUEST,
                format!(
                    "a budget is between 1 and {}µs of CPU, 1 and {} rows, and at most {} bytes of memory",
                    self.budget.cpu, self.budget.rows, self.budget.memory
                ),
            ));
        }
        Ok(self
            .terms
            .set_budget(&self.store, service, tool, budget)
            .await?)
    }

    /// Call cron tool `tool` of `service` when `cron` fires, from its next
    /// firing when it changes; `None` stops it. Kept until a deploy's config
    /// sets another.
    pub async fn set_cron(
        &self,
        service: &str,
        tool: &str,
        cron: Option<String>,
    ) -> Result<(), Denied> {
        self.deployed(service)?;
        let now = (self.now)() as i64;
        let due = cron
            .as_deref()
            .map(|cron| cron::next(cron, now))
            .transpose()
            .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
        let (scope, name) = host::split(service);
        if !table::set_cron(&self.store, scope, name, tool, cron, due).await? {
            return Err(deny(
                StatusCode::NOT_FOUND,
                format!("the service has no cron tool named {tool:?}"),
            ));
        }
        self.waking.notify_one();
        Ok(())
    }

    /// Each var of `service`'s `[vars]`, with the value it holds.
    pub async fn vars(&self, service: &str) -> Result<Vec<Setting>, Denied> {
        let deployed = self.deployed(service)?;
        let (scope, name) = host::split(service);
        let mut held = vars::list(&self.store, scope, name).await?;
        Ok(deployed
            .manifest
            .deps
            .vars
            .iter()
            .map(|(var, declared)| Setting {
                name: var.clone(),
                description: declared.description.clone(),
                default: declared.default.clone(),
                value: held.remove(var),
            })
            .collect())
    }

    /// Set `service`'s value of `var`, read from its next call; `None`
    /// returns it to its default. Kept until a deploy sets another or its
    /// `[vars]` drops the var.
    pub async fn set_var(
        &self,
        service: &str,
        var: &str,
        value: Option<String>,
    ) -> Result<(), Denied> {
        let deployed = self.deployed(service)?;
        let deps = &deployed.manifest.deps;
        if !deps.vars.contains_key(var) {
            return Err(deny(
                StatusCode::NOT_FOUND,
                format!("the service has no var named {var:?}"),
            ));
        }
        if let Some(value) = &value {
            deps.settable(var, value)
                .map_err(|e| deny(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
        }
        let (scope, name) = host::split(service);
        vars::set(&self.store, scope, name, var.to_owned(), value).await?;
        Ok(())
    }

    /// Refused unless `service` is deployed and exports `tool`.
    fn exported(&self, service: &str, tool: &str) -> Result<(), Denied> {
        let deployed = self.deployed(service)?;
        if !deployed.tools().any(|exported| exported == tool) {
            return Err(deny(
                StatusCode::NOT_FOUND,
                format!("the service has no tool named {tool:?}"),
            ));
        }
        Ok(())
    }

    /// `service`'s commit count.
    pub async fn commits(&self, service: &str) -> Result<i64, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        service::commits(&self.store, scope, name)
            .await?
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such service"))
    }

    /// Set or clear a deprecated tool's sunset.
    pub async fn sunset(&self, service: &str, tool: &str, at: Option<i64>) -> Result<(), Denied> {
        let (scope, name) = host::split(service);
        if !usage::sunset(&self.store, scope, name, tool, at).await? {
            return Err(deny(
                StatusCode::CONFLICT,
                format!("{tool} is not a deprecated tool of this service"),
            ));
        }
        Ok(())
    }

    /// Who uses `service` on `days`, counted from the Unix epoch, or over the
    /// last [`usage::WINDOW`] days without them.
    pub async fn usage(
        &self,
        service: &str,
        days: Option<Range<i32>>,
    ) -> Result<Vec<Usage>, Denied> {
        let (scope, name) = host::split(service);
        let mut used = usage::read(
            &self.store,
            scope,
            name,
            None,
            days.unwrap_or_else(|| usage::window(usage::seconds(&self.now))),
            self.usage.unsaved(service),
        )
        .await?;
        if let Some(deployed) = self.get(service) {
            for row in used.iter_mut().filter(|row| row.kind == Kind::Tool) {
                row.public = Some(deployed.public(&row.item));
            }
        }
        Ok(used)
    }

    /// `service`'s logs on `page`, oldest first, and its `Link` header. At
    /// most `page.limit`, held to [`MAX_PAGE`].
    pub async fn logs(
        &self,
        service: &str,
        mut page: wire::Page,
    ) -> Result<(Vec<Logged>, Option<String>), Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        page.limit = page.limit.clamp(1, MAX_PAGE);
        let logs = logs::read(
            &self.store,
            scope,
            name,
            page.clone(),
            self.logs.unsaved(service),
        )
        .await?;
        let link = logs
            .last()
            .filter(|_| logs.len() as i64 == page.limit)
            .map(|last| page.next(last.seq as i64));
        Ok((logs, link))
    }

    /// `service`'s cron tools.
    pub async fn crons(&self, service: &str) -> Result<wire::Crons, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        Ok(wire::Crons {
            crons: table::crons(&self.store, scope, name, self.unsaved(service)).await?,
            held: self.holds.holds(scope),
        })
    }

    /// The wakes armed to call `service`'s tools that have not run.
    pub async fn armed(&self, service: &str) -> Result<wire::Wakes, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        Ok(wire::Wakes {
            wakes: table::armed(&self.store, scope, name, self.unsaved(service)).await?,
            held: self.holds.holds(scope),
        })
    }

    /// The written calls and logs past `page`'s cursors, oldest first, those
    /// up to them deleted first.
    ///
    /// A call is answered only once every run with a smaller id is written,
    /// and a log is deleted only with the call it was emitted in. A failed
    /// call's input and tape stay.
    pub async fn records(&self, page: wire::RecordsPage) -> anyhow::Result<wire::Records> {
        records::page(&self.store, page, self.calls.floor()).await
    }

    /// `service`'s runs on `page`, newest first, and its `Link` header. At
    /// most `page.limit`, held to [`MAX_PAGE`].
    pub async fn calls(
        &self,
        service: &str,
        mut page: wire::CallPage,
    ) -> Result<(Vec<wire::Call>, Option<String>), Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        page.limit = page.limit.clamp(1, MAX_PAGE);
        let calls: Vec<wire::Call> = calls::read(
            &self.store,
            scope,
            Some(name),
            page.clone(),
            self.unsaved(service),
        )
        .await?
        .into_iter()
        .map(|ran| ran.call)
        .collect();
        let link = calls
            .last()
            .filter(|_| calls.len() as i64 == page.limit)
            .map(|last| page.next(last.id));
        Ok((calls, link))
    }

    /// The runs of `scope`'s services, of `page.service` alone when it names
    /// one, newest first, below the id `before`, a page at a time, with the
    /// next page's `Link`.
    pub async fn scope_calls(
        &self,
        scope: &str,
        mut page: wire::CallPage,
    ) -> Result<(Vec<wire::ServiceCall>, Option<String>), Denied> {
        page.limit = page.limit.clamp(1, MAX_PAGE);
        let unsaved = calls::Unsaved {
            ran: self.calls.unsaved_in(scope),
            emitted: self.logs.unsaved_in(scope),
        };
        let service = page.service.clone();
        let calls = calls::read(
            &self.store,
            scope,
            service.as_deref(),
            page.clone(),
            unsaved,
        )
        .await?;
        let link = calls
            .last()
            .filter(|_| calls.len() as i64 == page.limit)
            .map(|last| page.next(last.call.id));
        Ok((calls, link))
    }

    /// What `service`'s calls on `page` count to, by time and by facet, in
    /// [`wire::CallStats::bucket`]s of the range it asks for, its last 30
    /// days through now without one.
    pub async fn call_stats(
        &self,
        service: &str,
        page: wire::CallPage,
    ) -> Result<wire::CallStats, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        let (page, bucket) = self.ranged(page);
        Ok(calls::stats(
            &self.store,
            scope,
            Some(name),
            page,
            bucket,
            self.unsaved(service),
        )
        .await?)
    }

    /// [`Host::call_stats`] of every service in `scope`, of `page.service`
    /// alone when it names one.
    pub async fn scope_call_stats(
        &self,
        scope: &str,
        page: wire::CallPage,
    ) -> Result<wire::CallStats, Denied> {
        let (page, bucket) = self.ranged(page);
        let unsaved = calls::Unsaved {
            ran: self.calls.unsaved_in(scope),
            emitted: self.logs.unsaved_in(scope),
        };
        Ok(calls::stats(&self.store, scope, None, page, bucket, unsaved).await?)
    }

    /// `page` with its range set, the last 30 days through now where it
    /// names none, and the bucket length that range is counted in.
    fn ranged(&self, mut page: wire::CallPage) -> (wire::CallPage, u64) {
        let now = (self.now)();
        // `until` is excluded, so a call begun now is in the range.
        let until = page.until.unwrap_or(now + 1);
        let since = page
            .since
            .unwrap_or(until.saturating_sub(usage::WINDOW as u64 * 86_400_000));
        (page.since, page.until) = (Some(since), Some(until));
        (page, wire::CallStats::bucket(since, until))
    }

    /// What `service`'s call `id` ran with. Only a call that failed keeps it.
    /// The calls `service`'s run `id` made past its first.
    pub async fn call_nested(
        &self,
        service: &str,
        id: u64,
    ) -> Result<Vec<wire::NestedCall>, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        calls::nested(&self.store, scope, name, id, self.calls.unsaved(service))
            .await?
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such call"))
    }

    pub async fn call_input(&self, service: &str, id: u64) -> Result<wire::CallInput, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        match calls::input(&self.store, scope, name, id, self.calls.unsaved(service)).await? {
            Some(Some(input)) => Ok(input),
            Some(None) => Err(deny(
                StatusCode::NOT_FOUND,
                "the call kept no input; only a call that failed does",
            )),
            None => Err(deny(StatusCode::NOT_FOUND, "no such call")),
        }
    }

    /// The image `service`'s call `id` ran, pulled as the service's images
    /// are: from the registry it is deployed from when this is read, or kept
    /// here for an uploaded one. Only a call that failed names one.
    pub async fn call_image(&self, service: &str, id: u64) -> Result<Vec<u8>, Denied> {
        let input = self.call_input(service, id).await?;
        let (scope, name) = host::split(service);
        let row = service::row(&self.store, scope, name)
            .await?
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such service"))?;
        let reference = match Stored::uploaded(&row.reference, &row.digest) {
            true => &input.digest,
            false => &row.reference,
        };
        self.images
            .pull(reference, &input.digest)
            .await
            .map_err(|e| deny(StatusCode::BAD_GATEWAY, format!("{e:#}")))
    }

    /// `service`'s call `id`'s tape, encoded. Only a call that failed keeps
    /// one.
    pub async fn call_tape(&self, service: &str, id: u64) -> Result<Vec<u8>, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        match calls::tape(&self.store, scope, name, id, self.calls.unsaved(service)).await? {
            Some(Some(tape)) => Ok(tape),
            Some(None) => Err(deny(
                StatusCode::NOT_FOUND,
                "the call kept no tape; only a call that failed does",
            )),
            None => Err(deny(StatusCode::NOT_FOUND, "no such call")),
        }
    }

    /// A call from a service on a peer, run as `link` and committed here. A
    /// refusal is `403` with its reason.
    pub async fn link(
        &self,
        service: &str,
        tool: &str,
        args: Vec<u8>,
        link: Link,
    ) -> Result<Reply, Denied> {
        let deployed = self.get(service).ok_or_else(|| {
            deny(
                StatusCode::NOT_FOUND,
                format!("no service named {service:?} is deployed"),
            )
        })?;
        let refused = |message: String| {
            self.usage.record(
                service,
                Kind::Tool,
                tool,
                Source::Service,
                &link.from,
                Outcome::Refused,
            );
            Err(deny(StatusCode::FORBIDDEN, message))
        };
        if self.holds.holds(host::split(service).0) {
            return refused(format!("{service} is held"));
        }
        if self.stops.stopped(service) {
            return refused(format!("{service} is stopped"));
        }
        if !deployed.public(tool) {
            return refused(format!("{service} has no public tool named {tool:?}"));
        }
        if link.caller.is_some() && !deployed.reached_by_services(tool) {
            return refused(format!("{service}.{tool} takes no calls from services"));
        }
        let _slot = self.gate.acquire().await.map_err(anyhow::Error::from)?;
        let meters = Arc::new(Meters::default());
        let outcome = deployed
            .link(
                berm::Link {
                    caller: link
                        .caller
                        .as_deref()
                        .map(|caller| berm::Caller::Service(caller.into())),
                    user: link.user.as_ref().map(host::user),
                    client: link.client.as_deref().map(Into::into),
                    depth: link.depth,
                    query: link.query,
                    deadline: Instant::now() + Duration::from_millis(link.timeout),
                    stack: link
                        .stack
                        .iter()
                        .map(|service| Arc::from(service.as_str()))
                        .collect(),
                },
                tool,
                args,
                meters.clone(),
            )
            .await;
        let used = meters.take();
        self.metered(used.clone());
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => match error.downcast::<Refused>() {
                Ok(Refused(message)) => return refused(message),
                Err(error) => return Err(error.into()),
            },
        };
        if matches!(outcome.result, Output::Done(_)) {
            self.commits.commit(outcome.changes, None).await?;
        }
        let ended = Outcome::of(&outcome.result, outcome.denied);
        self.usage.record(
            service,
            Kind::Tool,
            tool,
            Source::Service,
            &link.from,
            ended,
        );
        Ok(Reply {
            result: outcome.result,
            denied: outcome.denied,
            meters: used.into_iter().fold(
                BTreeMap::new(),
                |mut meters: BTreeMap<String, BTreeMap<String, Meter>>, ((service, tool), used)| {
                    meters
                        .entry(service.to_string())
                        .or_default()
                        .insert(tool.to_string(), used);
                    meters
                },
            ),
        })
    }

    /// Call `service`'s `#[tool(stripe)]` tool with `event`, a Stripe event as its
    /// daemon took it, with no caller, and commit what it did. The host reads
    /// none of the event; the daemon verifies it. A held scope's services take
    /// none, and a tool's failure is a `500` carrying its message.
    pub async fn stripe(&self, service: &str, event: Vec<u8>) -> Result<(), Denied> {
        let deployed = self.deployed(service)?;
        let Some(tool) = deployed
            .manifest
            .tools
            .iter()
            .find(|spec| spec.stripe)
            .map(|spec| spec.name.clone())
        else {
            return Err(deny(
                StatusCode::NOT_FOUND,
                "the service has no stripe tool",
            ));
        };
        let paused = match () {
            _ if self.holds.holds(host::split(service).0) => Some("held"),
            _ if self.stops.stopped(service) => Some("stopped"),
            _ => None,
        };
        if let Some(paused) = paused {
            self.usage.record(
                service,
                Kind::Tool,
                &tool,
                Source::Stripe,
                "",
                Outcome::Refused,
            );
            return Err(deny(
                StatusCode::SERVICE_UNAVAILABLE,
                format!("{service} is {paused}"),
            ));
        }
        if self.pools.pool(host::split(service).0).full() {
            return Err(deny(
                StatusCode::TOO_MANY_REQUESTS,
                "the account holds as much memory as it may; retry",
            ));
        }
        let _slot = self.gate.acquire().await.map_err(anyhow::Error::from)?;
        let started = self.calls.start();
        let recorder = Recorder::default();
        let kept = event.clone();
        let invoked = self
            .invoke(
                &deployed,
                started.id,
                None,
                None,
                None,
                &tool,
                event,
                Taping::Record(recorder.clone()),
            )
            .await;
        let outcome = match &invoked {
            Ok(invoked) => Outcome::of(&invoked.result, invoked.denied),
            Err(_) => Outcome::Failed,
        };
        self.usage
            .record(service, Kind::Tool, &tool, Source::Stripe, "", outcome);
        self.calls.record(
            service,
            started,
            &tool,
            Source::Stripe,
            "",
            outcome,
            invoked.as_ref().ok(),
            (outcome == Outcome::Failed)
                .then(|| calls::ran_with(&tool, kept, None, &deployed.digest)),
            (outcome == Outcome::Failed)
                .then(|| recorder.tape())
                .as_ref(),
        );
        let invoked = invoked?;
        match invoked.result {
            Output::Done(_) => Ok(()),
            Output::Failed(failure) => Err(deny(StatusCode::INTERNAL_SERVER_ERROR, failure)),
        }
    }

    /// Every service's meters on yesterday and today.
    pub async fn recent(&self) -> Result<Vec<(String, Metered)>, Denied> {
        let since = self.today() - 1;
        Ok(meter::since(&self.store, since, self.meters.days(since)).await?)
    }

    /// `service`'s meters over the last 30 days, oldest first.
    pub async fn meters(&self, service: &str) -> Result<Vec<Metered>, Denied> {
        self.deployed(service)?;
        let (scope, name) = host::split(service);
        let today = self.today();
        let unsaved = self
            .meters
            .days_of(scope, Some(name), today - usage::WINDOW);
        Ok(meter::read(&self.store, scope, name, today, unsaved).await?)
    }

    /// What `service` is using now.
    pub async fn live(&self, service: &str) -> Result<wire::Live, Denied> {
        let deployed = self.deployed(service)?;
        let (scope, name) = host::split(service);
        let now = usage::seconds(&self.now);
        let day = now.div_euclid(86_400) as i32;
        let unsaved = self.meters.days_of(scope, Some(name), i32::MIN);
        let used = meter::used(&self.store, scope, name, day, unsaved).await?;
        let elapsed = (now - day as i64 * 86_400).max(1) as f64;
        let count = |total: u64, on: u64| wire::Count {
            total,
            per_second: on as f64 / elapsed,
        };
        let (running, memory) = deployed.running.now();
        Ok(wire::Live {
            running,
            memory,
            stored: meter::stored(&self.store, service, &self.databases, deployed.bytes).await?,
            cpu: count(used.all.cpu, used.on.cpu),
            read: count(used.all.read, used.on.read),
            written: count(used.all.written, used.on.written),
            proxied: count(used.all.proxied, used.on.proxied),
            logged: count(used.all.logged, used.on.logged),
        })
    }

    /// Hold `routes` in place of the routes held.
    pub async fn route(&self, routes: Vec<Route>) -> Result<(), Denied> {
        Ok(self.routes.replace(routes).await?)
    }

    /// Limit each of `limits`, by scope, to that many bytes of memory held at
    /// once, and every other scope to `Config::held`. A scope at its limit is
    /// refused a chain from outside or a Stripe event with `429`, and a wake
    /// fires again later; an invocation growing past it traps, and a deploy
    /// whose images would hold more is refused.
    pub fn limit(&self, limits: HashMap<String, u64>) {
        self.pools.replace(limits);
    }

    /// The memory `scope`'s services hold now, each and together, and its
    /// limit.
    pub fn memory(&self, scope: &str) -> wire::Memory {
        let services = self
            .list(scope)
            .into_iter()
            .map(|(name, deployed)| {
                let (_, running) = deployed.running.now();
                let held = wire::Held {
                    loaded: deployed.bytes,
                    running,
                };
                (name, held)
            })
            .collect();
        wire::Memory {
            services,
            ..self.pools.pool(scope).now()
        }
    }

    /// Hold `scopes` in place of the scopes held. A held scope's services fire
    /// no wakes and take no calls from services or peers; what they stored and
    /// what was due stays, and runs once the scope is released. Calls from
    /// outside are the daemon's to refuse.
    pub async fn hold(&self, scopes: Vec<String>) -> Result<(), Denied> {
        self.holds.replace(scopes.into_iter().collect()).await?;
        self.waking.notify_one();
        Ok(())
    }

    /// The scopes held.
    pub fn held(&self) -> Vec<String> {
        self.holds.list()
    }

    /// Add uses a peer counted of services this host runs.
    pub async fn record(&self, used: Vec<Used>) -> Result<(), Denied> {
        Ok(usage::record(&self.store, used).await?)
    }

    /// The service `service` runs now.
    pub(crate) fn deployed(&self, service: &str) -> Result<Arc<Service>, Denied> {
        self.get(service)
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such service"))
    }
}
