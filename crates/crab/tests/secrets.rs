//! The local host's secrets: kept by service, replaced atomically, sent
//! only to their hosts, and still there when the file is opened again.

use anyhow::Result;
use berm_host::Secrets;
use crab_agent::host::secrets::{FILE, Grants};

#[tokio::test]
async fn a_secret_is_kept_by_service_until_revoked() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(FILE);
    let grants = Grants::open(path.clone());
    assert_eq!(grants.grant("app", "TOKEN", "api.example.com").await?, None);

    let hosts = ["api.example.com".to_owned()];
    grants.set("app", "TOKEN", "one", &hosts)?;
    grants.set("app", "TOKEN", "two", &hosts)?;
    assert_eq!(
        grants.grant("app", "TOKEN", "api.example.com").await?,
        Some("two".to_owned())
    );
    assert_eq!(
        grants.grant("other", "TOKEN", "api.example.com").await?,
        None
    );
    assert_eq!(grants.grant("app", "TOKEN", "evil.example").await?, None);
    assert_eq!(grants.names("app").await?, ["TOKEN"]);

    let reopened = Grants::open(path);
    assert_eq!(
        reopened.grant("app", "TOKEN", "api.example.com").await?,
        Some("two".to_owned())
    );
    assert!(reopened.remove("app", "TOKEN")?);
    assert!(!reopened.remove("app", "TOKEN")?);
    assert!(reopened.names("app").await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_hostless_secret_is_provisioned_but_cannot_be_sent() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let grants = Grants::open(dir.path().join(FILE));
    grants.set("app", "TOKEN", "one", &[])?;
    assert_eq!(grants.grant("app", "TOKEN", "api.example.com").await?, None);
    assert_eq!(grants.names("app").await?, ["TOKEN"]);
    Ok(())
}
