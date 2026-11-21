use crate::state::commit::Entered;
use anyhow::Result;
use berm::{Changes, Scope};
use std::{future::Future, pin::Pin};

impl Scope for Entered {
    fn complete(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            if let Some(admitted) = &mut self.admitted {
                admitted.complete().await?;
            }
            if let Some(caller) = &self.caller {
                caller.complete().await?;
            }
            Ok(())
        })
    }

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
                caller,
                admitted,
                first,
            } = *self;
            let took = held.took().map(str::to_owned);
            if let Some(caller) = caller {
                caller.commit().await?;
            }
            held.commit().await?;
            let changes = if first {
                commits.commit(Changes::default(), took).await?;
                changes
            } else {
                commits.commit(changes, took).await?;
                Changes::default()
            };
            match admitted {
                Some(admitted) => admitted.close(changes).await,
                None => Ok(changes),
            }
        })
    }
}
