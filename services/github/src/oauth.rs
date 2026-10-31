//! OAuth code exchange and the profile it authorizes. The consuming app declares
//! GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET for github.com; repository tokens are not used.
use crate::{User, api};
use alloc::{
    format,
    string::{String, ToString},
};
use berm_lang::{Result, http, serde::Deserialize};

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Granted {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Exchange a code and read its user's profile, without returning or retaining the token.
/// The app must verify OAuth state before calling this function.
pub fn user(code: &str, redirect: &str) -> Result<User> {
    let form = http::form(&[("code", code), ("redirect_uri", redirect)]);
    let body = format!(
        "{form}&client_id={{secret:GITHUB_CLIENT_ID}}&client_secret={{secret:GITHUB_CLIENT_SECRET}}"
    );
    let response = http::fetch(
        "POST",
        "https://github.com/login/oauth/access_token",
        &[
            ("Accept", "application/json"),
            ("Content-Type", "application/x-www-form-urlencoded"),
        ],
        body.as_bytes(),
    )?;
    let granted: Granted = response.json().map_err(|_| {
        format!(
            "GitHub answered {} with an unreadable OAuth response",
            response.status
        )
    })?;
    let token = match granted.access_token {
        Some(access) if response.is_success() => access,
        _ => {
            return Err(format!(
                "GitHub refused the code: {}",
                granted
                    .error_description
                    .or(granted.error)
                    .unwrap_or_else(|| response.status.to_string())
            )
            .into());
        }
    };
    let profile = api::fetch(
        "GET",
        "https://api.github.com/user",
        &[],
        &format!("Bearer {token}"),
    )?;
    api::read(&profile)
}
