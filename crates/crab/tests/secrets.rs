//! The local host's secrets: kept by service, set once until revoked, and
//! still there when the file is opened again.

use anyhow::Result;
use berm_host::Secrets;
use crab_agent::host::secrets::{FILE, Grants};

#[tokio::test]
async fn a_secret_is_kept_by_service_until_revoked() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(FILE);
    let grants = Grants::open(path.clone());
    assert_eq!(grants.grant("app", "TOKEN", "api.example.com").await?, None);

    assert!(grants.set("app", "TOKEN", "one")?);
    assert!(!grants.set("app", "TOKEN", "two")?);
    assert_eq!(
        grants.grant("app", "TOKEN", "api.example.com").await?,
        Some("one".to_owned())
    );
    assert_eq!(
        grants.grant("other", "TOKEN", "api.example.com").await?,
        None
    );
    assert_eq!(grants.names("app").await?, ["TOKEN"]);

    let reopened = Grants::open(path);
    assert_eq!(
        reopened.grant("app", "TOKEN", "api.example.com").await?,
        Some("one".to_owned())
    );
    assert!(reopened.remove("app", "TOKEN")?);
    assert!(!reopened.remove("app", "TOKEN")?);
    assert!(reopened.names("app").await?.is_empty());
    Ok(())
}
