//! ID tokens the host signs with a service's key, which the guest never holds.

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use berm_api::Output;
use berm_host::test::{self, TestHost};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde_json::{Value, json};

/// When the host's clock stands, in milliseconds.
const NOW: u64 = 1_700_000_000_123;

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(NOW).await?;
    host.deploy("fixture", &test::build("berm-fixture", "fixture")?)
        .await?;
    Ok(host)
}

/// How `token` ended when asked to sign `claims`.
async fn signed(host: &TestHost, claims: Value) -> Result<Output> {
    let args = json!({ "claims": claims.to_string() });
    Ok(host
        .call("fixture", "token", args.to_string().as_bytes())
        .await?
        .result)
}

fn decoded(part: &str) -> Result<Value> {
    Ok(serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part)?)?)
}

#[tokio::test]
async fn a_token_verifies_against_the_service_key_and_names_the_host_issuer() -> Result<()> {
    let host = deployed().await?;
    let claims = json!({
        "sub": "user-1",
        "aud": "https://app.example.com",
        "exp": 1_700_000_600,
        "iss": "https://forged.example.com",
        "iat": 1,
        "nonce": "n-0",
    });
    let Output::Done(answer) = signed(&host, claims).await? else {
        panic!("the claims are signed");
    };
    let answer: Value = serde_json::from_str(&answer)?;
    let issuer = host.issuer("fixture").await?;
    assert_eq!(answer["issuer"], issuer);

    let token = answer["token"].as_str().context("a token")?;
    let (input, signature) = token.rsplit_once('.').context("a signature")?;
    UnparsedPublicKey::new(&ED25519, host.public_key("fixture")?)
        .verify(input.as_bytes(), &URL_SAFE_NO_PAD.decode(signature)?)
        .map_err(|_| anyhow::anyhow!("the signature does not verify"))?;

    let (header, payload) = input.split_once('.').context("a payload")?;
    let header = decoded(header)?;
    assert_eq!(header["alg"], "EdDSA");
    assert_eq!(header["typ"], "JWT");
    assert!(header["kid"].as_str().is_some_and(|kid| !kid.is_empty()));
    let payload = decoded(payload)?;
    assert_eq!(payload["iss"], issuer);
    assert_eq!(payload["iat"], NOW / 1000);
    assert_eq!(payload["sub"], "user-1");
    assert_eq!(payload["aud"], "https://app.example.com");
    assert_eq!(payload["exp"], 1_700_000_600);
    assert_eq!(payload["nonce"], "n-0");
    Ok(())
}

#[tokio::test]
async fn each_service_signs_with_its_own_key() -> Result<()> {
    let host = deployed().await?;
    host.deploy("other", &test::build("berm-fixture", "fixture")?)
        .await?;
    assert_ne!(host.public_key("fixture")?, host.public_key("other")?);
    assert_ne!(host.issuer("fixture").await?, host.issuer("other").await?);
    Ok(())
}

#[tokio::test]
async fn claims_without_sub_aud_or_exp_are_refused() -> Result<()> {
    let host = deployed().await?;
    for (claims, missing) in [
        (json!({"aud": "a", "exp": 1}), "`sub`"),
        (json!({"sub": "s", "exp": 1}), "`aud`"),
        (json!({"sub": "s", "aud": [], "exp": 1}), "`aud`"),
        (json!({"sub": "s", "aud": "a"}), "`exp`"),
        (json!({"sub": "s", "aud": "a", "exp": "1"}), "`exp`"),
        (json!(["s"]), "not a JSON object"),
    ] {
        let Output::Failed(failure) = signed(&host, claims.clone()).await? else {
            panic!("{claims} is refused");
        };
        assert!(failure.contains(missing), "{claims}: {failure}");
    }
    Ok(())
}
