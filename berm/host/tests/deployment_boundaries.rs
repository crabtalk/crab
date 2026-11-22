use anyhow::Result;
use axum::http::StatusCode;
use berm_abi::{ABI_SECTION, SERVICE_SECTION};
use berm_api::{
    Image, Output,
    host::{Caller, DATABASE_SIZE},
};
use berm_build::fixture;
use berm_host::{Denied, test::TestHost};
use serde_json::{Value, json};

#[tokio::test]
async fn breaking_deploy_counts_successful_usage_for_thirty_days_even_in_a_group() -> Result<()> {
    const DAY: u64 = 86_400_000;
    let host = TestHost::open(100 * DAY).await?;
    let old = fixture::image("berm-fixture")?;
    let records: Vec<_> =
        serde_json::Deserializer::from_slice(Image(&old).section(ABI_SECTION)?.unwrap())
            .into_iter::<Value>()
            .collect::<Result<_, _>>()?;
    let removed: Vec<u8> = records
        .iter()
        .filter(|r| r["tool"]["name"] != "note")
        .map(serde_json::to_vec)
        .collect::<Result<Vec<_>, _>>()?
        .concat();
    let new = Image(&old).replace_section(ABI_SECTION, &removed)?;
    let deploy = |image| {
        [
            ("a", image, DATABASE_SIZE),
            ("b", old.as_slice(), DATABASE_SIZE),
        ]
    };
    host.deploy_set("group", &deploy(&old)).await?;
    let caller: Caller = serde_json::from_value(json!({"id":"customer"}))?;
    let call = |word| {
        host.host.call(
            "b",
            Some(&caller),
            "ask_note",
            serde_json::to_vec(&json!({"target":"a","word":word})).unwrap(),
            false,
        )
    };
    assert!(matches!(call("fail").await?.1, Output::Failed(_)));
    host.deploy_set("group", &deploy(&new)).await?;
    host.deploy_set("group", &deploy(&old)).await?;
    assert!(matches!(call("used").await?.1, Output::Done(_)));
    host.advance(29 * DAY);
    let denied = host.deploy_set("group", &deploy(&new)).await.unwrap_err();
    assert_eq!(
        denied.downcast_ref::<Denied>().unwrap().status,
        StatusCode::CONFLICT
    );
    assert!(denied.to_string().contains("note"), "{denied:#}");
    host.advance(DAY);
    host.deploy_set("group", &deploy(&new)).await?;
    Ok(())
}

#[tokio::test]
async fn failed_later_member_cannot_restore_an_image_behind_applied_migrations() -> Result<()> {
    let host = TestHost::open(0).await?;
    let old = fixture::image("berm-keeper")?;
    let manifest: Value = serde_json::from_slice(Image(&old).section(SERVICE_SECTION)?.unwrap())?;
    let mut migrated = manifest.clone();
    migrated["version"] = json!("2.0.0");
    migrated["database"]["migrations"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "name":"zzz.sql","sql":"CREATE TABLE only_in_v2 (value INTEGER)"
        }));
    let migrated = Image(&old).replace_section(SERVICE_SECTION, &serde_json::to_vec(&migrated)?)?;
    let mut broken = manifest;
    broken["database"]["migrations"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "name":"zzz.sql","sql":"This migration is invalid SQL"
        }));
    let broken = Image(&old).replace_section(SERVICE_SECTION, &serde_json::to_vec(&broken)?)?;
    host.deploy_set("group", &[("a", &old, DATABASE_SIZE)])
        .await?;
    let failure = host
        .deploy_set(
            "group",
            &[
                ("a", &migrated, DATABASE_SIZE),
                ("b", &broken, DATABASE_SIZE),
            ],
        )
        .await
        .unwrap_err();
    assert!(failure.to_string().contains("zzz.sql"), "{failure:#}");
    assert_eq!(host.host.idl("a")?.version, "2.0.0");
    assert!(host.host.get("b").is_none());
    Ok(())
}
