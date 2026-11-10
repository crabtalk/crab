//! Running a tool as the first link of a chain.

use crate::{env::Env, host::Host};
use anyhow::{Context, Result};
use berm::{ByTool, Client, Meter, Meters, Service, User, chain, tape::Taping};
use berm_api::{Output, host::NestedCall};
use std::sync::Arc;

impl<E: Env> Host<E> {
    /// Direct execution bypasses outside-call admission and retains runtime errors.
    /// The embedder supplies and verifies the caller identity.
    pub async fn execute(
        &self,
        service: &str,
        tool: &str,
        args: Vec<u8>,
        caller: Option<&str>,
        user: Option<User>,
        client: Client,
    ) -> Result<Invoked> {
        let deployed = self
            .get(service)
            .with_context(|| format!("no service named {service:?} is deployed"))?;
        self.invoke(
            &deployed,
            self.calls.start().id,
            caller,
            None,
            user,
            client,
            tool,
            args,
            Taping::Off,
        )
        .await
    }

    /// Run `tool` on `service` as the first link of a chain, as call `call`,
    /// and commit what it wrote. A writing call into a service with a database
    /// commits that database when it returns. What the chain emits is kept
    /// beside `call` whatever it returns.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn invoke(
        &self,
        deployed: &Service,
        call: u64,
        caller: Option<&str>,
        key: Option<&str>,
        user: Option<User>,
        client: Client,
        tool: &str,
        args: Vec<u8>,
        taping: Taping,
    ) -> Result<Invoked> {
        let meters = Arc::new(Meters::default());
        let chain = chain::next();
        self.logs.started(chain, call);
        let outcome = deployed
            .call_as(
                caller.map(Into::into),
                key.map(Into::into),
                user,
                client,
                tool,
                args,
                meters.clone(),
                chain,
                taping,
            )
            .await;
        self.logs.ended(chain);
        let nested = self.calls.take_nested(call);
        let meters = meters.take();
        let own = meters
            .iter()
            .filter(|((service, _), _)| *service == deployed.name)
            .fold(Meter::default(), |mut own, (_, used)| {
                own += *used;
                own
            });
        let meter = self.metered(meters);
        let outcome = outcome?;
        if matches!(outcome.result, Output::Failed(_)) {
            return Ok(Invoked {
                result: outcome.result,
                denied: outcome.denied,
                meter,
                own,
                nested,
            });
        }
        self.commits.commit(outcome.changes, None).await?;
        Ok(Invoked {
            result: outcome.result,
            denied: false,
            meter,
            own,
            nested,
        })
    }

    /// Count a chain's `meters` against the tools of the services this host
    /// runs, and answer their sum over every service the chain reached.
    pub(crate) fn metered(&self, meters: ByTool) -> Meter {
        let mut total = Meter::default();
        for ((service, tool), used) in meters {
            if self.berm.get(&service).is_some() {
                self.meters.record(&service, &tool, used);
            }
            total += used;
        }
        total
    }
}

/// How a call ended.
pub struct Invoked {
    pub result: Output,
    /// The tool refused its caller; `result` is then [`Output::Failed`].
    pub denied: bool,
    /// What the chain used, over every service it reached.
    pub meter: Meter,
    /// This service's usage across its invocations in the chain.
    pub own: Meter,
    /// The calls the run made past its first, on this host.
    pub nested: Arc<[NestedCall]>,
}
