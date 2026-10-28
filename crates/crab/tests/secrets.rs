//! The local host's secrets: kept by service, set once until revoked, sent
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
    assert!(grants.set("app", "TOKEN", "one", &hosts)?);
    assert!(!grants.set("app", "TOKEN", "two", &hosts)?);
    assert_eq!(
        grants.grant("app", "TOKEN", "api.example.com").await?,
        Some("one".to_owned())
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
        Some("one".to_owned())
    );
    assert!(reopened.remove("app", "TOKEN")?);
    assert!(!reopened.remove("app", "TOKEN")?);
    assert!(reopened.names("app").await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_secret_goes_only_to_its_hosts_once_it_has_any() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let grants = Grants::open(dir.path().join(FILE));
    assert!(grants.set("app", "TOKEN", "one", &[])?);
    assert_eq!(grants.grant("app", "TOKEN", "api.example.com").await?, None);
    assert!(grants.names("app").await?.is_empty());

    assert!(grants.set_hosts("app", "TOKEN", &["api.example.com".to_owned()])?);
    assert!(!grants.set_hosts("app", "OTHER", &["api.example.com".to_owned()])?);
    assert_eq!(
        grants.grant("app", "TOKEN", "api.example.com").await?,
        Some("one".to_owned())
    );
    assert_eq!(grants.grant("app", "TOKEN", "example.com").await?, None);
    assert_eq!(grants.names("app").await?, ["TOKEN"]);
    Ok(())
}
