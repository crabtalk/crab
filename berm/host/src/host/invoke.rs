//! Running a tool as the first link of a chain.

use crate::{env::Env, host::Host};
use anyhow::Result;
use berm::{ByTool, Meter, Meters, Program, User, tape::Taping};
use berm_api::{Output, host::NestedCall};
use std::sync::Arc;

impl<E: Env> Host<E> {
    /// Run `tool` on `program` as the first link of a chain, as call `call`,
    /// and commit what it wrote. A writing call into a service with a database
    /// commits that database when it returns. What the chain emits is kept
    /// beside `call` whatever it returns.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn invoke(
        &self,
        program: &Program,
        call: u64,
        caller: Option<&str>,
        user: Option<User>,
        client: Option<Arc<str>>,
        tool: &str,
        args: Vec<u8>,
        taping: Taping,
    ) -> Result<Invoked> {
        let meters = Arc::new(Meters::default());
        let chain = berm::chain();
        self.logs.started(chain, call);
        let outcome = program
            .call_as(
                caller.map(Into::into),
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
            .filter(|((service, _), _)| *service == program.name)
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
pub(crate) struct Invoked {
    pub result: Output,
    /// `result` is [`Output::Failed`] because the tool refused its caller.
    pub denied: bool,
    /// What the chain used, over every service it reached.
    pub meter: Meter,
    /// What the tool's own invocation used, as its budget counts it.
    pub own: Meter,
    /// The calls the run made past its first, on this host.
    pub nested: Arc<[NestedCall]>,
}
