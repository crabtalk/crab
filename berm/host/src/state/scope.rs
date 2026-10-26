use crate::state::commit::Entered;
use anyhow::Result;
use berm::{Changes, Scope};
use std::{future::Future, pin::Pin};

impl Scope for Entered {
    fn own(&self) -> bool {
        true
    }

    fn close(
        self: Box<Self>,
        changes: Changes,
    ) -> Pin<Box<dyn Future<Output = Result<Changes>> + Send>> {
        Box::pin(async move {
            let Entered {
                commits,
                held,
                first,
            } = *self;
            let took = held.took().map(str::to_owned);
            held.commit().await?;
            if first {
                commits.commit(Changes::default(), took).await?;
                return Ok(changes);
            }
            commits.commit(changes, took).await?;
            Ok(Changes::default())
        })
    }
}
