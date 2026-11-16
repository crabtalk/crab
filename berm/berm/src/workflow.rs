use berm_api::workflow::spend::Context;
use std::future::Future;

tokio::task_local! {
    static SPEND: Context;
}

pub fn current() -> Option<Context> {
    SPEND.try_with(Clone::clone).ok()
}

pub async fn scope<F: Future>(context: Context, work: F) -> F::Output {
    SPEND.scope(context, work).await
}
