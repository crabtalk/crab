//! Signing in and linking identities, against arranged GitHub and Google
//! answers.

use anyhow::{Context, Result};
use berm_api::Output;
use berm_host::test::{self, TestHost};
use serde_json::{Value, json};

const GITHUB_TOKEN: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER: &str = "https://api.github.com/user";
const GOOGLE_TOKEN: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER: &str = "https://openidconnect.googleapis.com/v1/userinfo";

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(1_700_000_000_000).await?;
    host.deploy("account", &test::build("berm-account")?)
        .await?;
    for (secret, to) in [
        ("GITHUB_CLIENT_ID", "github.com"),
        ("GITHUB_CLIENT_SECRET", "github.com"),
        ("GOOGLE_CLIENT_ID", "oauth2.googleapis.com"),
        ("GOOGLE_CLIENT_SECRET", "oauth2.googleapis.com"),
    ] {
        host.grant("account", secret, to, &secret.to_lowercase());
    }
    Ok(host)
}

fn github(host: &TestHost, id: u64) {
    host.arrange("POST", GITHUB_TOKEN, 200, r#"{"access_token":"gho_1"}"#);
    host.arrange(
        "GET",
        GITHUB_USER,
        200,
        json!({"id": id, "login": "octo", "name": null, "email": "octo@example.com", "avatar_url": "https://avatars.example.com/octo"})
            .to_string(),
    );
}

fn google(host: &TestHost, sub: &str, verified: bool) {
    host.arrange("POST", GOOGLE_TOKEN, 200, r#"{"access_token":"ya29.1"}"#);
    host.arrange(
        "GET",
        GOOGLE_USER,
        200,
        json!({"sub": sub, "name": "Ada", "email": "ada@example.com", "email_verified": verified, "picture": "https://pictures.example.com/ada"})
            .to_string(),
    );
}

/// What `tool` answered, read as JSON.
async fn done(host: &TestHost, tool: &str, args: Value) -> Result<Value> {
    match host
        .call("account", tool, args.to_string().as_bytes())
        .await?
        .result
    {
        Output::Done(answer) => Ok(serde_json::from_str(&answer)?),
        Output::Failed(failure) => anyhow::bail!("{tool} failed: {failure}"),
    }
}

async fn failed(host: &TestHost, tool: &str, args: Value) -> Result<String> {
    match host
        .call("account", tool, args.to_string().as_bytes())
        .await?
        .result
    {
        Output::Failed(failure) => Ok(failure),
        Output::Done(answer) => anyhow::bail!("{tool} answered {answer}"),
    }
}

fn code() -> Value {
    json!({"code": "c0de", "redirect": "https://app.example.com/callback"})
}

/// `args` with `session`'s token beside them.
fn with(session: &Value, mut args: Value) -> Value {
    args["session"] = session["token"].clone();
    args
}

#[tokio::test]
async fn a_github_sign_in_makes_an_account_once() -> Result<()> {
    let host = deployed().await?;
    github(&host, 42);
    let first = done(&host, "github", code()).await?;
    let token = first["token"].as_str().context("a token")?;
    assert_eq!(token.len(), 64, "{token}");
    assert_eq!(first["expires"], 1_700_000_000 + 30 * 24 * 60 * 60);
    let account = done(&host, "account", with(&first, json!({}))).await?;
    assert_eq!(account["id"], "1");
    assert_eq!(account["identities"][0]["name"], "octo");
    assert_eq!(account["identities"][0]["email"], "octo@example.com");

    let again = done(&host, "github", code()).await?;
    assert_ne!(again["token"], first["token"]);
    assert_eq!(
        done(&host, "account", with(&again, json!({}))).await?["id"],
        "1"
    );

    let exchange = &host.fetched()[0];
    assert_eq!(
        String::from_utf8(exchange.body.clone())?,
        "code=c0de&redirect_uri=https%3A%2F%2Fapp.example.com%2Fcallback&client_id=github_client_id&client_secret=github_client_secret"
    );
    Ok(())
}

#[tokio::test]
async fn a_google_sign_in_keeps_only_a_verified_email() -> Result<()> {
    let host = deployed().await?;
    google(&host, "g-1", false);
    let mut args = code();
    args["verifier"] = "v3rifier".into();
    let session = done(&host, "google", args).await?;
    let account = done(&host, "account", with(&session, json!({}))).await?;
    assert_eq!(account["identities"][0]["name"], "Ada");
    assert_eq!(account["identities"][0]["email"], Value::Null);
    let exchange = String::from_utf8(host.fetched()[0].body.clone())?;
    assert!(exchange.contains("code_verifier=v3rifier"), "{exchange}");
    assert!(
        exchange.ends_with("client_id=google_client_id&client_secret=google_client_secret"),
        "{exchange}"
    );
    Ok(())
}

#[tokio::test]
async fn a_refused_code_reads_as_the_provider_said() -> Result<()> {
    let host = deployed().await?;
    host.arrange(
        "POST",
        GITHUB_TOKEN,
        200,
        r#"{"error":"bad_verification_code","error_description":"The code passed is incorrect or expired."}"#,
    );
    let failure = failed(&host, "github", code()).await?;
    assert!(
        failure.contains("The code passed is incorrect or expired."),
        "{failure}"
    );
    Ok(())
}

#[tokio::test]
async fn a_linked_identity_signs_in_to_the_same_account() -> Result<()> {
    let host = deployed().await?;
    github(&host, 42);
    let session = done(&host, "github", code()).await?;

    google(&host, "g-1", true);
    let account = done(&host, "link_google", with(&session, code())).await?;
    assert_eq!(account["id"], "1");
    let providers: Vec<&str> = account["identities"]
        .as_array()
        .context("identities")?
        .iter()
        .filter_map(|identity| identity["provider"].as_str())
        .collect();
    assert_eq!(providers, ["github", "google"]);

    let google = done(&host, "google", code()).await?;
    assert_eq!(
        done(&host, "account", with(&google, json!({}))).await?["id"],
        "1"
    );
    Ok(())
}

#[tokio::test]
async fn an_identity_of_another_account_is_not_linked() -> Result<()> {
    let host = deployed().await?;
    github(&host, 42);
    let session = done(&host, "github", code()).await?;
    google(&host, "g-1", true);
    done(&host, "google", code()).await?;

    let failure = failed(&host, "link_google", with(&session, code())).await?;
    assert!(failure.contains("another account"), "{failure}");
    Ok(())
}

#[tokio::test]
async fn a_session_ends_when_signed_out_or_expired() -> Result<()> {
    let host = deployed().await?;
    github(&host, 42);
    let session = done(&host, "github", code()).await?;
    let stranger = json!({"session": "0".repeat(64)});
    let failure = failed(&host, "account", stranger).await?;
    assert!(failure.contains("not signed in"), "{failure}");

    assert_eq!(
        done(&host, "sign_out", with(&session, json!({}))).await?,
        true
    );
    assert_eq!(
        done(&host, "sign_out", with(&session, json!({}))).await?,
        false
    );
    let failure = failed(&host, "account", with(&session, json!({}))).await?;
    assert!(failure.contains("not signed in"), "{failure}");

    let expiring = done(&host, "github", code()).await?;
    host.set_now(1_700_000_000_000 + 31 * 24 * 60 * 60 * 1000);
    let failure = failed(&host, "account", with(&expiring, json!({}))).await?;
    assert!(failure.contains("not signed in"), "{failure}");
    Ok(())
}
