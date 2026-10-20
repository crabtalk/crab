//! An Ed25519 key a service's ID tokens are signed with, and what a relying
//! party reads to verify them.

use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use serde_json::{Value, json};

/// One signing key.
pub struct Key {
    pair: Ed25519KeyPair,
    kid: String,
}

impl Key {
    /// A fresh key, beside the PKCS#8 document that holds it.
    pub fn generate() -> Result<(Self, Vec<u8>)> {
        let document = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
            .map_err(|e| anyhow!("generating a key failed: {e}"))?;
        let document = document.as_ref().to_vec();
        Ok((Self::from_pkcs8(&document)?, document))
    }

    /// The key a PKCS#8 document [`Key::generate`] answered holds.
    pub fn from_pkcs8(document: &[u8]) -> Result<Self> {
        let pair =
            Ed25519KeyPair::from_pkcs8(document).map_err(|e| anyhow!("not an Ed25519 key: {e}"))?;
        let thumbprint = format!(
            r#"{{"crv":"Ed25519","kty":"OKP","x":"{}"}}"#,
            base64url(pair.public_key().as_ref())
        );
        let kid = base64url(digest::digest(&digest::SHA256, thumbprint.as_bytes()).as_ref());
        Ok(Self { pair, kid })
    }

    /// Its id: the RFC 7638 thumbprint of its JWK, SHA-256, base64url.
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// Its public key's 32 bytes.
    pub fn public(&self) -> &[u8] {
        self.pair.public_key().as_ref()
    }

    /// An Ed25519 signature over `message`.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.pair.sign(message).as_ref().to_vec()
    }

    /// Its public half, as one key of a JWK Set.
    pub fn jwk(&self) -> Value {
        json!({
            "kty": "OKP",
            "crv": "Ed25519",
            "x": base64url(self.public()),
            "kid": self.kid,
            "alg": "EdDSA",
            "use": "sig",
        })
    }
}

/// The OpenID Provider metadata of `issuer`, whose keys are the JWK Set at
/// `jwks_uri`.
pub fn discovery(issuer: &str, jwks_uri: &str) -> Value {
    json!({
        "issuer": issuer,
        "jwks_uri": jwks_uri,
        "id_token_signing_alg_values_supported": ["EdDSA"],
        "subject_types_supported": ["public"],
        "response_types_supported": ["id_token"],
    })
}

/// `bytes` in base64url without padding (RFC 7515).
pub(crate) fn base64url(bytes: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}
