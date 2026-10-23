//! The local host's `secrets.toml`: one table by secret name, for any program
//! and any host its manifest declares, read again when it changes.

use anyhow::Result;
use berm_host::Secrets;
use crab_agent::host::secrets::{FILE, Grants};
use std::{fs, thread, time::Duration};

#[tokio::test]
async fn a_secret_is_granted_by_name_and_read_again_when_the_file_changes() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(FILE);
    let grants = Grants::open(path.clone());
    assert_eq!(grants.grant("app", "TOKEN", "api.example.com").await?, None);

    fs::write(&path, "TOKEN = \"one\"\n")?;
    assert_eq!(
        grants.grant("app", "TOKEN", "api.example.com").await?,
        Some("one".to_owned())
    );
    assert_eq!(
        grants.grant("other", "TOKEN", "elsewhere.example").await?,
        Some("one".to_owned())
    );
    assert_eq!(grants.names("app").await?, ["TOKEN"]);

    // A modification time that moves on, on filesystems that keep seconds.
    thread::sleep(Duration::from_millis(1100));
    fs::write(&path, "TOKEN = \"two\"\nOTHER = \"x\"\n")?;
    assert_eq!(
        grants.grant("app", "TOKEN", "api.example.com").await?,
        Some("two".to_owned())
    );
    assert_eq!(grants.names("app").await?, ["OTHER", "TOKEN"]);
    Ok(())
}
