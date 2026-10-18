use crate::{
    host,
    state::{commit::Entered, service},
};
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
                dry,
            } = *self;
            if let Some((chain, keep)) = dry {
                let Some(keep) = keep else {
                    if let Some(kept) = commits.kept.lock().get_mut(&chain) {
                        kept.held.push(held);
                        for write in changes.writes {
                            kept.writes
                                .insert((write.program.to_string(), write.key), write.value);
                        }
                    }
                    return Ok(Changes::default());
                };
                drop((keep, held));
                return Ok(match first {
                    true => changes,
                    false => Changes::default(),
                });
            }
            let took = held.took().map(str::to_owned);
            held.commit().await?;
            if let Some(key) = took {
                let (scope, name) = host::split(&key);
                let (scope, name) = (scope.to_owned(), name.to_owned());
                commits
                    .store
                    .run(move |conn| {
                        let tx = conn.transaction()?;
                        service::commit(&tx, &scope, &name, &[])?;
                        tx.commit()
                    })
                    .await?;
            }
            if first {
                return Ok(changes);
            }
            commits.commit(&changes).await?;
            Ok(Changes::default())
        })
    }
}
