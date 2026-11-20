//! Historical image imports and ABI boundary refusals.

use anyhow::Result;
use berm::{Berm, Calls, Config, Engine, MissingHostCall};
use berm_api::{Image, host::DATABASE_SIZE};
use berm_host::{Denied, test::TestHost};
use std::collections::BTreeSet;

const FIXTURE: &[u8] = include_bytes!("../old/fixture_abi7.wasm");
const KEEPER: &[u8] = include_bytes!("../old/keeper_abi7.wasm");

/// Every host call that existed when these images were built.
const CALLS: [&str; 25] = [
    "berm.args.len",
    "berm.args.read",
    "berm.call",
    "berm.call.after",
    "berm.caller",
    "berm.client",
    "berm.deny",
    "berm.done",
    "berm.emit",
    "berm.fail",
    "berm.get",
    "berm.http.fetch",
    "berm.http.fetch_all",
    "berm.http.proxy",
    "berm.http.proxy_all",
    "berm.limit",
    "berm.now",
    "berm.random",
    "berm.result.read",
    "berm.set",
    "berm.sql",
    "berm.trace",
    "berm.user",
    "berm.var",
    "berm.vouch",
];

#[test]
fn the_images_import_every_host_call() -> Result<()> {
    let imports: BTreeSet<String> = [FIXTURE, KEEPER]
        .into_iter()
        .map(|image| Image(image).imports())
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(imports, BTreeSet::from(CALLS.map(str::to_owned)));
    Ok(())
}

#[tokio::test]
async fn legacy_images_are_refused_before_applying_the_new_access_default() -> Result<()> {
    let host = TestHost::open(0).await?;
    for image in [FIXTURE, KEEPER] {
        let error = host
            .deploy("old", image)
            .await
            .err()
            .expect("old ABI refused");
        let denied = error.downcast_ref::<Denied>().unwrap();
        assert_eq!(denied.status.as_u16(), 400);
        assert!(denied.message.contains("ABI version 7"));
    }
    Ok(())
}

#[test]
fn a_runtime_links_a_call_it_has_no_door_for() -> Result<()> {
    let engine = Engine::new(&Config::default())?;
    let berm = Berm::new(&engine, Calls::by_name(Calls::DEFAULT_DEPTH), Vec::new());
    berm.deploy("fixture", &berm_build::fixture::image("berm-fixture")?)?;
    Ok(())
}

/// `image` importing `berm.sez`, a call no host serves, in place of `berm.set`.
fn unknown(image: &[u8]) -> Vec<u8> {
    let at = image
        .windows(8)
        .position(|name| name == b"berm.set")
        .expect("the import");
    let mut image = image.to_vec();
    image[at + 7] = b'z';
    image
}

#[test]
fn a_runtime_refuses_an_image_naming_a_call_it_has_never_heard_of() -> Result<()> {
    let engine = Engine::new(&Config::default())?;
    let berm = Berm::new(&engine, Calls::by_name(Calls::DEFAULT_DEPTH), Vec::new());
    let Err(refused) = berm.deploy("keeper", &unknown(KEEPER)) else {
        panic!("no runtime serves berm.sez");
    };
    let missing = refused.downcast_ref::<MissingHostCall>();
    assert!(missing.is_some_and(|missing| missing.0 == "berm.sez"));
    Ok(())
}

#[tokio::test]
async fn a_project_deploy_of_a_call_the_host_lacks_carries_its_code() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = unknown(&berm_build::fixture::image("berm-keeper")?);
    let refused = host
        .deploy_set("p", &[("acct/keeper", &image, DATABASE_SIZE)])
        .await
        .err()
        .expect("no host serves berm.sez");
    let denied = refused.downcast_ref::<Denied>().expect("a refusal");
    assert_eq!(denied.status.as_u16(), 422);
    assert_eq!(denied.explain, Some("E0020"));
    Ok(())
}

#[tokio::test]
async fn a_deploy_of_a_call_the_host_lacks_carries_its_code() -> Result<()> {
    let host = TestHost::open(0).await?;
    let Err(refused) = host
        .deploy(
            "keeper",
            &unknown(&berm_build::fixture::image("berm-keeper")?),
        )
        .await
    else {
        panic!("no host serves berm.sez");
    };
    let denied = refused.downcast_ref::<Denied>().expect("a refusal");
    assert_eq!(denied.status.as_u16(), 422);
    assert_eq!(denied.explain, Some("E0020"));
    assert!(denied.message.contains("berm.sez"));
    Ok(())
}
