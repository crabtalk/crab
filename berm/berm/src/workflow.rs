use berm_abi::workflow::Run;
use berm_api::workflow::spend::Context;
use std::{future::Future, sync::Arc};

tokio::task_local! {
    static SPEND: Context;
    static STEP: Arc<Run>;
}

pub fn current() -> Option<Context> {
    SPEND.try_with(Clone::clone).ok()
}

pub async fn scope<F: Future>(context: Context, work: F) -> F::Output {
    SPEND.scope(context, work).await
}

/// The step `work` runs as: its first invocation answers `berm.workflow.run`
/// with `run`.
pub async fn step<F: Future>(run: Run, work: F) -> F::Output {
    STEP.scope(Arc::new(run), work).await
}

pub(crate) fn step_current() -> Option<Arc<Run>> {
    STEP.try_with(Clone::clone).ok()
}
