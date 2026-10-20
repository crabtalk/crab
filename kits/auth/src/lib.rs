//! Sign-in as a kit: a GitHub or Google login turned into an ID token the
//! program using it issues, for an account each of the user's identities signs
//! in to. Its tables are `auth_accounts` and `auth_identities`.

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use berm_lang::Schema;

mod account;
mod provider;

berm_lang::kit!(
    database = "migrations",
    secrets = [
        "GITHUB_CLIENT_ID",
        "GITHUB_CLIENT_SECRET",
        "GOOGLE_CLIENT_ID",
        "GOOGLE_CLIENT_SECRET",
    ],
    syscalls = {
        "berm.token.sign",
        "berm.http.fetch" = [
            "github.com",
            "api.github.com",
            "oauth2.googleapis.com",
            "openidconnect.googleapis.com",
        ],
    },
);

/// An ID token this service issued.
#[derive(Schema)]
pub struct Signed {
    /// A compact JWT: `sub` the account's id, `aud` this service's issuer.
    pub token: String,
    /// When it expires, in seconds since the Unix epoch.
    pub expires: u64,
}

/// An account, the `sub` of the tokens issued for it.
#[derive(Schema)]
pub struct Account {
    pub id: String,
    pub identities: Vec<Identity>,
}

/// Who a user is to a provider, as it last answered.
#[derive(Schema)]
pub struct Identity {
    /// `github` or `google`.
    pub provider: String,
    /// The user's id at the provider.
    pub subject: String,
    pub name: Option<String>,
    /// Their email, when the provider shows one: GitHub's public email, and
    /// Google's when it is verified.
    pub email: Option<String>,
    /// The URL of their picture.
    pub avatar: Option<String>,
}

/// Why a call is refused.
#[berm_lang::error_code]
pub enum AuthError {
    #[msg("this call carries no user this service signed in")]
    NotSignedIn,
    #[msg("that identity signs in to another account")]
    Taken,
}
