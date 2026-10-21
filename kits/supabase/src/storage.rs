//! Objects in one Supabase Storage bucket.

use crate::send;
use alloc::{format, string::String, vec::Vec};
use berm_lang::{Result, http};

/// A Storage bucket. An object's path is relative to the bucket, its segments
/// separated by `/`. A path with an empty, `.` or `..` segment is refused and
/// sends nothing.
pub struct Bucket {
    url: String,
}

impl Bucket {
    pub(crate) fn new(url: String) -> Self {
        Self { url }
    }

    /// Stores `body` at `path` as `content_type`. With `upsert`, an object
    /// already at `path` is replaced; without, it fails the upload.
    pub fn upload(&self, path: &str, content_type: &str, body: &[u8], upsert: bool) -> Result<()> {
        let upsert = if upsert { "true" } else { "false" };
        send(
            "POST",
            &self.object(path)?,
            &[("Content-Type", content_type), ("x-upsert", upsert)],
            body,
        )?;
        Ok(())
    }

    /// The bytes of the object at `path`.
    pub fn download(&self, path: &str) -> Result<Vec<u8>> {
        send("GET", &self.object(path)?, &[], &[])
    }

    /// Deletes the object at `path`.
    pub fn delete(&self, path: &str) -> Result<()> {
        send("DELETE", &self.object(path)?, &[], &[])?;
        Ok(())
    }

    /// The URL of the object at `path`, each segment percent-encoded.
    fn object(&self, path: &str) -> Result<String> {
        if path
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."))
        {
            return Err(format!("{path:?} is not an object path").into());
        }
        let segments: Vec<String> = path.split('/').map(http::encode).collect();
        Ok(format!("{}/{}", self.url, segments.join("/")))
    }
}
