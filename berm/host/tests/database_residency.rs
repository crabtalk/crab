use anyhow::Result;
use berm::{Callsite, Changes, Client, Scope, Service};
use berm_api::Output;
use berm_build::fixture;
use berm_host::{Config, test::TestHost};
use rusqlite::Connection;
use serde_json::json;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;

struct Pause {
    started: Arc<Notify>,
    resume: Arc<Notify>,
    wait: bool,
}
impl Scope for Pause {
    fn own(&self) -> bool {
        false
    }
    fn complete(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            if self.wait {
                self.started.notify_one();
                self.resume.notified().await;
            }
            Ok(())
        })
    }
    fn close(
        self: Box<Self>,
        changes: Changes,
    ) -> Pin<Box<dyn Future<Output = Result<Changes>> + Send>> {
        Box::pin(async move { Ok(changes) })
    }
}

#[tokio::test]
async fn connection_eviction_preserves_active_nested_transactions_and_rollback_order() -> Result<()>
{
    let (started, resume) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let (began, release) = (started.clone(), resume.clone());
    let armed = AtomicBool::new(true);
    let enter = Arc::new(move |service: &Service, _: &Callsite| {
        let scope = Pause {
            started: began.clone(),
            resume: release.clone(),
            wait: service.name.as_ref() == "outer" && armed.swap(false, Ordering::Relaxed),
        };
        Box::pin(async move { Ok(Box::new(scope) as Box<dyn Scope>) })
            as Pin<Box<dyn Future<Output = Result<Box<dyn Scope>>> + Send>>
    });
    let host = TestHost::open_with_scope(0, Config::default(), Some(enter)).await?;
    let image = fixture::image("berm-keeper")?;
    host.deploy("outer", &image).await?;
    host.deploy("inner", &image).await?;
    let runtime = host.host.clone();
    let call = tokio::spawn(async move {
        runtime
            .execute(
                "outer",
                "relay",
                br#"{"word":"kept"}"#.to_vec(),
                None,
                None,
                Client::default(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(10), started.notified()).await?;
    for i in 0..100 {
        host.host
            .databases
            .get(&format!("idle-{i}"))?
            .limit(65536)
            .await?;
    }
    assert!(host.state("outer", "last").await?.is_none());
    resume.notify_one();
    call.await??;
    assert_eq!(host.state("outer", "last").await?, Some(b"kept".to_vec()));
    assert_eq!(host.state("inner", "last").await?, Some(b"kept".to_vec()));
    let connection = Connection::open(host.host.databases.path("outer"))?;
    connection.execute_batch("CREATE TRIGGER reject AFTER INSERT ON words WHEN NEW.word='reject' BEGIN SELECT RAISE(FAIL,'rejected'); END")?;
    let failed = host.call("outer", "put", br#"{"word":"reject"}"#).await?;
    assert!(matches!(failed.result, Output::Failed(_)));
    let accepted = host.call("outer", "put", br#"{"word":"accepted"}"#).await?;
    assert!(matches!(accepted.result, Output::Done(_)));
    assert_eq!(host.call_json("outer", "count", json!({})).await?, 1);

    Ok(())
}
