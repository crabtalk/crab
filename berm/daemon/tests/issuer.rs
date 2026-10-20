//! Each program's issuer: its discovery document and JWK Set served over HTTP,
//! and its key kept under the root.

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use berm::Manifest;
use berm_api::host;
use berm_host::Issuer;
use bermd::{Keys, Service};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::net::TcpListener;

/// A service under a root of its own, serving at the URL it answers.
async fn served() -> Result<(String, PathBuf)> {
    let root = std::env::temp_dir().join(format!("bermd-issuer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    let service = Service::new(root.clone(), 4, None, &url).await?;
    let image = berm_host::test::build("berm-fixture", "fixture")?;
    let public: Vec<String> = Manifest::from_image(&image)?
        .tools
        .into_iter()
        .filter(|tool| tool.public)
        .map(|tool| tool.name)
        .collect();
    service
        .deploy("fixture", image, &public, true, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    tokio::spawn(service.clone().serve(listener, std::future::pending()));
    Ok((url, root))
}

async fn get(url: &str) -> Result<(u16, Value)> {
    let response = reqwest::get(url).await?;
    let status = response.status().as_u16();
    Ok((status, serde_json::from_str(&response.text().await?)?))
}

#[tokio::test]
async fn a_program_token_verifies_against_its_served_keys_and_the_key_is_kept() -> Result<()> {
    let (url, root) = served().await?;
    let issuer = format!("{url}/issuers/fixture");

    let (status, discovery) = get(&format!("{issuer}/.well-known/openid-configuration")).await?;
    assert_eq!(status, 200);
    assert_eq!(discovery["issuer"], issuer);
    assert_eq!(
        discovery["id_token_signing_alg_values_supported"],
        json!(["EdDSA"])
    );
    let jwks_uri = discovery["jwks_uri"].as_str().context("a jwks_uri")?;
    let (status, jwks) = get(jwks_uri).await?;
    assert_eq!(status, 200);
    let jwk = &jwks["keys"][0];
    assert_eq!(jwk["kty"], "OKP");
    assert_eq!(jwk["crv"], "Ed25519");
    assert_eq!(jwk["alg"], "EdDSA");
    assert_eq!(jwk["use"], "sig");

    let args = json!({"claims": json!({"sub": "u", "aud": "a", "exp": 1}).to_string()});
    let called = reqwest::Client::new()
        .post(format!("{url}/programs/fixture/tools/token"))
        .header(host::CALLER, r#"{"id":"alice"}"#)
        .body(args.to_string())
        .send()
        .await?
        .text()
        .await?;
    let called: Value = serde_json::from_str(&called)?;
    let answer: Value = serde_json::from_str(called["done"].as_str().context("done")?)?;
    assert_eq!(answer["issuer"], issuer);
    let token = answer["token"].as_str().context("a token")?;
    let (input, signature) = token.rsplit_once('.').context("a signature")?;
    let header: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD.decode(input.split('.').next().context("a header")?)?,
    )?;
    assert_eq!(header["kid"], jwk["kid"]);
    let x = URL_SAFE_NO_PAD.decode(jwk["x"].as_str().context("an x")?)?;
    UnparsedPublicKey::new(&ED25519, x)
        .verify(input.as_bytes(), &URL_SAFE_NO_PAD.decode(signature)?)
        .map_err(|_| anyhow::anyhow!("the signature does not verify"))?;

    assert!(root.join("keys/fixture.p8").exists());
    let reopened = Keys::open(&root, &url);
    assert_eq!(reopened.kid("fixture").await?, jwk["kid"]);

    let (status, _) = get(&format!("{url}/issuers/absent/jwks")).await?;
    assert_eq!(status, 404);
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}
