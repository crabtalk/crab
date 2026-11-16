use crate::{schedule, workflow::config::Spec};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    pub service: String,
    pub workflow: String,
    pub revision: String,
    pub at: u64,
    pub input: Value,
}

impl Occurrence {
    pub const STALE: &'static str = "E0075";
    pub const WINDOW_MS: u64 = 60_000;

    pub fn id(&self) -> String {
        Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!(
                "crab:workflow:{}:{}:{}:{}",
                self.service, self.workflow, self.revision, self.at
            )
            .as_bytes(),
        )
        .to_string()
    }

    pub fn check(&self, spec: &Spec, now: u64) -> Result<()> {
        let configured = spec
            .config
            .schedule
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("workflow no longer has a schedule"))?;
        ensure!(
            self.workflow == spec.entry.name
                && self.revision == configured.revision()
                && self.input == configured.input,
            "workflow schedule changed"
        );
        ensure!(
            self.at <= now && now - self.at < Self::WINDOW_MS,
            "workflow occurrence is outside its UTC minute"
        );
        let before = i64::try_from(self.at)?
            .checked_sub(1)
            .ok_or_else(|| anyhow::anyhow!("invalid occurrence time"))?;
        ensure!(
            schedule::next(&configured.cron, before)? == self.at as i64,
            "time is not an occurrence of the workflow schedule"
        );
        configured.check_input(&spec.entry.graph.input)
    }
}
