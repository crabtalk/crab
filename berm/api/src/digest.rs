//! Content digests, in the form an OCI registry names a blob by.

use sha2::{Digest, Sha256};

/// What a digest's hex follows: its algorithm and a colon.
pub const PREFIX: &str = "sha256:";

/// The SHA-256 of `bytes`, in lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The digest whose hex is `hex`.
pub fn prefixed(hex: &str) -> String {
    format!("{PREFIX}{hex}")
}

/// The digest of `bytes`.
pub fn of(bytes: &[u8]) -> String {
    prefixed(&self::hex(bytes))
}
