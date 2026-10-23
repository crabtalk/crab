//! GitHub and Google, through `berm.http.fetch`: an authorization code
//! exchanged for an access token with the client credentials in the
//! service's secrets, and the identity that token reads.

use crate::Identity;
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{
    Result, http,
    serde::{Deserialize, de::DeserializeOwned},
};

const FORM: &str = "application/x-www-form-urlencoded";
/// GitHub refuses an API request without one.
const USER_AGENT: &str = "crabkit-auth";

/// What a token endpoint answers, a grant or an error.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Granted {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct GithubUser {
    id: u64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct GoogleUser {
    sub: String,
    name: Option<String>,
    email: Option<String>,
    email_verified: Option<bool>,
    picture: Option<String>,
}

impl Identity {
    /// The GitHub identity `code` was granted for.
    pub fn github(code: &str, redirect: &str) -> Result<Self> {
        let form = http::form(&[("code", code), ("redirect_uri", redirect)]);
        let access = exchange(
            "GitHub",
            "https://github.com/login/oauth/access_token",
            &format!(
                "{form}&client_id={{secret:GITHUB_CLIENT_ID}}&client_secret={{secret:GITHUB_CLIENT_SECRET}}"
            ),
        )?;
        let user: GithubUser = get(
            "GitHub",
            "https://api.github.com/user",
            &[
                ("Accept", "application/vnd.github+json"),
                ("Authorization", &format!("Bearer {access}")),
                ("User-Agent", USER_AGENT),
            ],
        )?;
        Ok(Self {
            provider: "github".to_owned(),
            subject: user.id.to_string(),
            name: user.name.or(Some(user.login)),
            email: user.email,
            avatar: user.avatar_url,
        })
    }

    /// The Google identity `code` was granted for.
    pub fn google(code: &str, redirect: &str, verifier: Option<&str>) -> Result<Self> {
        let mut fields = Vec::from([
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect),
        ]);
        if let Some(verifier) = verifier {
            fields.push(("code_verifier", verifier));
        }
        let form = http::form(&fields);
        let access = exchange(
            "Google",
            "https://oauth2.googleapis.com/token",
            &format!(
                "{form}&client_id={{secret:GOOGLE_CLIENT_ID}}&client_secret={{secret:GOOGLE_CLIENT_SECRET}}"
            ),
        )?;
        let user: GoogleUser = get(
            "Google",
            "https://openidconnect.googleapis.com/v1/userinfo",
            &[("Authorization", &format!("Bearer {access}"))],
        )?;
        Ok(Self {
            provider: "google".to_owned(),
            subject: user.sub,
            name: user.name,
            email: user.email.filter(|_| user.email_verified == Some(true)),
            avatar: user.picture,
        })
    }
}

/// The access token `provider`'s token endpoint at `url` grants for `form`.
fn exchange(provider: &str, url: &str, form: &str) -> Result<String> {
    let response = http::fetch(
        "POST",
        url,
        &[("Accept", "application/json"), ("Content-Type", FORM)],
        form.as_bytes(),
    )?;
    let granted: Granted = response.json().map_err(|_| {
        format!(
            "{provider} answered {} with what this service cannot read",
            response.status
        )
    })?;
    match granted.access_token {
        Some(access) if response.is_success() => Ok(access),
        _ => Err(format!(
            "{provider} refused the code: {}",
            granted
                .error_description
                .or(granted.error)
                .unwrap_or_else(|| response.status.to_string())
        )
        .into()),
    }
}

/// What `provider` answers a `GET` of `url`, read as `T`.
fn get<T: DeserializeOwned>(provider: &str, url: &str, headers: &[(&str, &str)]) -> Result<T> {
    let response = http::fetch("GET", url, headers, b"")?;
    if !response.is_success() {
        return Err(format!("{provider} answered {} for the user", response.status).into());
    }
    response
        .json()
        .map_err(|_| format!("{provider} answered a user this service cannot read").into())
}
