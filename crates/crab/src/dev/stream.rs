//! Each call the services take while `crab dev` runs, with the logs it emitted.

use crate::dev::{Dev, Event};
use berm_api::host::{CallPage, Page};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

/// How often the host is asked for new calls.
const EVERY: Duration = Duration::from_millis(500);

/// The most calls printed per interval; calls past it in one interval are not
/// printed.
const CALLS: i64 = 100;

/// The most logs printed under one call.
const LOGS: i64 = 1000;

impl Dev {
    /// Print every call begun since the dev host started, oldest first, each
    /// followed by its logs. Runs until the process ends.
    pub async fn stream(self: Arc<Self>) {
        let since = self.status().since;
        // The newest call printed, by service.
        let mut seen: BTreeMap<String, u64> = BTreeMap::new();
        let mut every = tokio::time::interval(EVERY);
        loop {
            every.tick().await;
            for watched in self.status().services {
                let name = watched.name;
                let page = CallPage {
                    tool: None,
                    event: None,
                    outcome: None,
                    since: Some(since),
                    until: None,
                    before: None,
                    limit: CALLS,
                };
                // A service not deployed yet has no calls to ask for.
                let Ok((calls, _)) = self.service.host.calls(&name, page).await else {
                    continue;
                };
                let last = seen.get(&name).copied().unwrap_or(0);
                for call in calls.iter().rev().filter(|call| call.id > last) {
                    self.emit(&Event::Call {
                        service: &name,
                        call,
                    });
                    let page = Page {
                        topic: None,
                        call: Some(call.id as i64),
                        after: 0,
                        limit: LOGS,
                    };
                    if let Ok((logs, _)) = self.service.host.logs(&name, page).await {
                        for log in &logs {
                            self.emit(&Event::Log {
                                service: &name,
                                log,
                            });
                        }
                    }
                    seen.insert(name.clone(), call.id);
                }
            }
        }
    }
}
