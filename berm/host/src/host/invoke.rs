//! Running a tool as the first link of a chain.

use crate::{env::Env, host::Host};
use anyhow::Result;
use berm::{Dry, Meter, Meters, Program, User};
use berm_api::Output;
use std::{collections::BTreeMap, sync::Arc};

impl<E: Env> Host<E> {
    /// Run `tool` on `program` as the first link of a chain, as call `call`,
    /// and commit what it wrote unless `dry` is set. A writing call into a
    /// service with a database commits that database when it returns. What the
    /// chain emits is kept beside `call` whatever it returns.
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
        dry: Option<Arc<Dry>>,
    ) -> Result<Invoked> {
        let meters = Arc::new(Meters::default());
        let keeps = dry.is_none();
        let chain = berm::chain();
        self.logs.started(chain, call);
        let outcome = program
            .call_as(
                caller.map(Into::into),
                user,
                client,
                tool,
                args,
                dry,
                meters.clone(),
                chain,
            )
            .await;
        self.logs.ended(chain);
        let meter = self.metered(meters.take());
        let outcome = outcome?;
        if matches!(outcome.result, Output::Failed(_)) || !keeps {
            return Ok(Invoked {
                result: outcome.result,
                denied: outcome.denied,
                meter,
            });
        }
        self.commits.commit(&outcome.changes).await?;
        Ok(Invoked {
            result: outcome.result,
            denied: false,
            meter,
        })
    }

    /// Count a chain's `meters` against the services this host runs, and
    /// answer their sum over every service the chain reached.
    pub(crate) fn metered(&self, meters: BTreeMap<Arc<str>, Meter>) -> Meter {
        let mut total = Meter::default();
        for (service, used) in meters {
            if self.berm.get(&service).is_some() {
                self.meters.record(&service, used);
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
}
