use anyhow::Result;
use berm::{Config, Engine};
use std::env;

#[test]
fn disk_cache_accepts_relative_and_absolute_paths_before_they_exist() -> Result<()> {
    let cwd = env::current_dir()?;
    let dir = tempfile::tempdir_in(&cwd)?;
    let relative = dir.path().strip_prefix(&cwd)?.join("relative");
    assert!(relative.is_relative());
    for cache in [relative, dir.path().join("absolute")] {
        Engine::new(&Config {
            cache_dir: Some(cache),
            instances: 1,
            ..Config::default()
        })?;
    }
    Ok(())
}
