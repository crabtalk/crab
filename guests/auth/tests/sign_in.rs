//! Signing in and linking identities, against arranged GitHub and Google
//! answers.

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use berm_api::Output;
use berm_host::test::{self, TestHost, User};
use serde_json::{Value, json};

const GITHUB_TOKEN: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER: &str = "https://api.github.com/user";
const GOOGLE_TOKEN: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER: &str = "https://openidconnect.googleapis.com/v1/userinfo";

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(1_700_000_000_000).await?;
    host.deploy("auth", &test::build("crab-guest-auth", "auth")?)
        .await?;
    for (secret, to) in [
        ("GITHUB_CLIENT_ID", "github.com"),
        ("GITHUB_CLIENT_SECRET", "github.com"),
        ("GOOGLE_CLIENT_ID", "oauth2.googleapis.com"),
        ("GOOGLE_CLIENT_SECRET", "oauth2.googleapis.com"),
    ] {
        host.grant("auth", secret, to, &secret.to_lowercase());
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
async fn done(host: &TestHost, tool: &str, args: Value, user: Option<User>) -> Result<Value> {
    let args = args.to_string();
    let ran = match user {
        Some(user) => host.call_for("auth", tool, args.as_bytes(), user).await?,
        None => host.call("auth", tool, args.as_bytes()).await?,
    };
    match ran.result {
        Output::Done(answer) => Ok(serde_json::from_str(&answer)?),
        Output::Failed(failure) => anyhow::bail!("{tool} failed: {failure}"),
    }
}

async fn failed(host: &TestHost, tool: &str, args: Value, user: Option<User>) -> Result<String> {
    let args = args.to_string();
    let ran = match user {
        Some(user) => host.call_for("auth", tool, args.as_bytes(), user).await?,
        None => host.call("auth", tool, args.as_bytes()).await?,
    };
    match ran.result {
        Output::Failed(failure) => Ok(failure),
        Output::Done(answer) => anyhow::bail!("{tool} answered {answer}"),
    }
}

/// The claims of the token `signed` carries.
fn claims(signed: &Value) -> Result<Value> {
    let token = signed["token"].as_str().context("a token")?;
    let payload = token.split('.').nth(1).context("a payload")?;
    Ok(serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload)?)?)
}

fn code() -> Value {
    json!({"code": "c0de", "redirect": "https://app.example.com/callback"})
}

async fn user(host: &TestHost, subject: &str) -> Result<User> {
    Ok(User {
        issuer: host.issuer("auth").await?.into(),
        subject: subject.into(),
    })
}

#[tokio::test]
async fn a_github_sign_in_makes_an_account_once() -> Result<()> {
    let host = deployed().await?;
    github(&host, 42);
    let first = done(&host, "github", code(), None).await?;
    let claims = claims(&first)?;
    let issuer = host.issuer("auth").await?;
    assert_eq!(claims["iss"], issuer);
    assert_eq!(claims["aud"], issuer);
    assert_eq!(claims["sub"], "1");
    assert_eq!(claims["name"], "octo");
    assert_eq!(claims["email"], "octo@example.com");
    assert_eq!(claims["exp"], first["expires"]);
    assert_eq!(first["expires"], 1_700_000_000 + 3600);

    let again = done(&host, "github", code(), None).await?;
    assert_eq!(self::claims(&again)?["sub"], "1");

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
    let signed = done(&host, "google", args, None).await?;
    let claims = claims(&signed)?;
    assert_eq!(claims["name"], "Ada");
    assert_eq!(claims["email"], Value::Null);
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
    let failure = failed(&host, "github", code(), None).await?;
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
    done(&host, "github", code(), None).await?;

    google(&host, "g-1", true);
    let account = done(&host, "link_google", code(), Some(user(&host, "1").await?)).await?;
    assert_eq!(account["id"], "1");
    let providers: Vec<&str> = account["identities"]
        .as_array()
        .context("identities")?
        .iter()
        .filter_map(|identity| identity["provider"].as_str())
        .collect();
    assert_eq!(providers, ["github", "google"]);

    let signed = done(&host, "google", code(), None).await?;
    assert_eq!(claims(&signed)?["sub"], "1");
    Ok(())
}

#[tokio::test]
async fn an_identity_of_another_account_is_not_linked() -> Result<()> {
    let host = deployed().await?;
    github(&host, 42);
    done(&host, "github", code(), None).await?;
    google(&host, "g-1", true);
    done(&host, "google", code(), None).await?;

    let failure = failed(&host, "link_google", code(), Some(user(&host, "1").await?)).await?;
    assert!(failure.contains("another account"), "{failure}");
    Ok(())
}

#[tokio::test]
async fn a_user_another_issuer_signed_in_has_no_account_here() -> Result<()> {
    let host = deployed().await?;
    github(&host, 42);
    done(&host, "github", code(), None).await?;

    let stranger = User {
        issuer: "https://accounts.google.com".into(),
        subject: "1".into(),
    };
    let failure = failed(&host, "account", json!({}), Some(stranger)).await?;
    assert!(
        failure.contains("no user this service signed in"),
        "{failure}"
    );
    let account = done(&host, "account", json!({}), Some(user(&host, "1").await?)).await?;
    assert_eq!(account["identities"][0]["subject"], "42");
    Ok(())
}
