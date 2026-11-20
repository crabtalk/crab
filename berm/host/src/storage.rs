use anyhow::Result;
use berm_api::storage::Key;
use futures_util::future::BoxFuture;
use std::path::Path;

pub trait Repository: Send + Sync {
    /// Load the current snapshot into `path` and return its revision.
    fn open<'a>(&'a self, key: &'a Key, path: &'a Path) -> BoxFuture<'a, Result<Option<String>>>;

    /// Publish a snapshot if `revision` is still current.
    fn commit<'a>(
        &'a self,
        key: &'a Key,
        revision: Option<&'a str>,
        schema: usize,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<String>>;
}
