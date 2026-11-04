//! `berm-kit`, a dependency of the keeper: its declarations in the
//! manifest, and its migrations applied and kept under its name.

use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    host.deploy("keeper", &fixture::image("berm-keeper")?)
        .await?;
    Ok(host)
}

#[tokio::test]
async fn the_manifest_carries_the_kit_and_its_declarations() -> Result<()> {
    let host = TestHost::open(0).await?;
    let service = host
        .deploy("keeper", &fixture::image("berm-keeper")?)
        .await?;
    let manifest = &service.manifest;
    assert_eq!(manifest.kits.len(), 1);
    assert_eq!(manifest.kits[0].name, "berm-kit");
    assert!(manifest.deps.secrets.contains(&"kit".to_owned()));
    assert_eq!(manifest.deps.http.fetch, ["kit.example.com"]);
    for name in ["berm.set", "berm.sql"] {
        assert!(
            manifest.imports.iter().any(|import| import == name),
            "{name}"
        );
    }
    for name in ["berm.get", "berm.http.fetch"] {
        assert!(
            !manifest.imports.iter().any(|import| import == name),
            "{name}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn the_kit_keeps_its_table_beside_the_service() -> Result<()> {
    let host = deployed().await?;
    for expected in [1, 2] {
        let ran = host.call("keeper", "note", br#"{"text":"hello"}"#).await?;
        let Output::Done(count) = ran.result else {
            panic!("note runs: {:?}", ran.result);
        };
        assert_eq!(count, expected.to_string());
    }
    let ran = host.call("keeper", "put", br#"{"word":"hi"}"#).await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    Ok(())
}

/// The kit's first migration and the service's share a name; each is kept
/// under its own, so both apply, and a redeploy finds both unchanged.
#[tokio::test]
async fn a_redeploy_finds_each_kits_migrations_applied() -> Result<()> {
    let host = deployed().await?;
    host.call("keeper", "note", br#"{"text":"kept"}"#).await?;
    host.deploy("keeper", &fixture::image("berm-keeper")?)
        .await?;
    let ran = host.call("keeper", "note", br#"{"text":"again"}"#).await?;
    let Output::Done(count) = ran.result else {
        panic!("note runs after a redeploy: {:?}", ran.result);
    };
    assert_eq!(count, "2");
    Ok(())
}
