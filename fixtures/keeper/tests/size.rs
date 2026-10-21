//! A deploy's database size: refused outside its bounds, and left as it was
//! by a refused deploy.

#![allow(clippy::err_expect)]

use anyhow::Result;
use berm_api::host::{DATABASE_SIZE, MIN_DATABASE_SIZE};
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_size_under_the_floor_is_refused() -> Result<()> {
    let image = test::build("berm-keeper", "keeper")?;
    let host = TestHost::open(0).await?;
    let error = host
        .sized("keeper", &image, MIN_DATABASE_SIZE - 1)
        .await
        .err()
        .expect("the size is refused");
    assert!(
        format!("{error:#}").contains(&MIN_DATABASE_SIZE.to_string()),
        "{error:#}"
    );
    host.sized("keeper", &image, MIN_DATABASE_SIZE).await?;
    Ok(())
}

#[tokio::test]
async fn a_refused_deploy_leaves_the_size_as_it_was() -> Result<()> {
    let image = test::build("berm-keeper", "keeper")?;
    let host = TestHost::open(0).await?;
    host.deploy("keeper", &image).await?;
    let (applied, changed) = (
        "CREATE TABLE words (word TEXT NOT NULL);",
        "CREATE TABLE wordz (word TEXT NOT NULL);",
    );
    let mut changed_image = image.clone();
    let mut found = 0;
    while let Some(at) = changed_image
        .windows(applied.len())
        .position(|window| window == applied.as_bytes())
    {
        changed_image[at..at + changed.len()].copy_from_slice(changed.as_bytes());
        found += 1;
    }
    assert!(found > 0, "the image carries its migration");

    let error = host
        .sized("keeper", &changed_image, MIN_DATABASE_SIZE << 1)
        .await
        .err()
        .expect("the changed migration is refused");
    assert!(
        format!("{error:#}").contains("has changed since"),
        "{error:#}"
    );
    let held = host
        .host()
        .databases
        .get("keeper")?
        .limit(DATABASE_SIZE)
        .await?;
    assert_eq!(held, DATABASE_SIZE);
    Ok(())
}
