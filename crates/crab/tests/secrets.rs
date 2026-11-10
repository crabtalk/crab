//! The local host's secrets: kept by service, replaced atomically, sent
//! only to their hosts, and still there when the file is opened again.

use anyhow::Result;
use berm_host::Secrets;
use crab_agent::host::secrets::{FILE, Grants};
use crabtalk_sdk::account::secret::{Create, Grant};

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

#[tokio::test]
async fn runtime_resources_survive_restarts_and_grants_rotate_and_revoke_independently()
-> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(FILE);
    std::fs::write(
        &path,
        "[legacy]\nTOKEN = { value = \"old-key\", hosts = [\"api.example.com\"], created_at = 1 }\n",
    )?;
    let store = Grants::open(path.clone());
    let resource = store.create_resource(&Create {
        name: "Research Bot".into(),
        value: "first-key".into(),
    })?;
    assert_eq!(
        store
            .grant("legacy", "TOKEN", "api.example.com")
            .await?
            .as_deref(),
        Some("old-key")
    );
    assert_eq!(
        store
            .grant("relay", &resource.id, "api.example.com")
            .await?,
        None
    );
    assert!(store.grant_resource(
        &resource.id,
        Grant {
            service: "relay".into(),
            hosts: vec!["api.example.com".into()]
        }
    )?);
    assert_eq!(
        store
            .grant("relay", &resource.id, "api.example.com")
            .await?
            .as_deref(),
        Some("first-key")
    );
    assert_eq!(
        store
            .grant("other", &resource.id, "api.example.com")
            .await?,
        None
    );
    assert_eq!(
        store.grant("relay", &resource.id, "other.example").await?,
        None
    );
    assert!(store.rotate_resource(&resource.id, "rotated-key")?);
    let reopened = Grants::open(path);
    assert_eq!(
        reopened
            .grant("relay", &resource.id, "api.example.com")
            .await?
            .as_deref(),
        Some("rotated-key")
    );
    assert!(!serde_json::to_string(&reopened.resources()?)?.contains("key"));
    assert!(reopened.revoke_resource(&resource.id, "relay")?);
    assert_eq!(
        reopened
            .grant("relay", &resource.id, "api.example.com")
            .await?,
        None
    );
    assert!(reopened.delete_resource(&resource.id)?);
    assert!(!reopened.rotate_resource(&resource.id, "resurrect")?);
    assert_eq!(
        reopened
            .grant("legacy", "TOKEN", "api.example.com")
            .await?
            .as_deref(),
        Some("old-key")
    );
    Ok(())
}
