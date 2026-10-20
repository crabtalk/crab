//! Sign-in as a service: a GitHub or Google login turned into an ID token this
//! service issues, for an account each of the user's identities signs in to.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use berm_lang::Schema;

mod account;
mod provider;

#[berm_lang::program(
    readme = "README.md",
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
)]
mod tools {
    use crate::{Account, Identity, Signed};
    use alloc::string::String;
    use berm_lang::Result;

    /// Signs in with a GitHub authorization code, creating the account on the
    /// identity's first sign-in, and answers a token for it.
    #[public]
    #[anonymous]
    pub fn github(
        /// The `code` GitHub sent the user back with.
        code: String,
        /// The `redirect_uri` the code was asked for with.
        redirect: String,
    ) -> Result<Signed> {
        Identity::github(&code, &redirect)?.sign_in()
    }

    /// Signs in with a Google authorization code, creating the account on the
    /// identity's first sign-in, and answers a token for it.
    #[public]
    #[anonymous]
    pub fn google(
        /// The `code` Google sent the user back with.
        code: String,
        /// The `redirect_uri` the code was asked for with.
        redirect: String,
        /// The PKCE `code_verifier`, when the code was asked for with a
        /// challenge.
        verifier: Option<String>,
    ) -> Result<Signed> {
        Identity::google(&code, &redirect, verifier.as_deref())?.sign_in()
    }

    /// Adds the GitHub identity behind an authorization code to the calling
    /// user's account, and answers the account.
    #[public]
    pub fn link_github(
        /// The `code` GitHub sent the user back with.
        code: String,
        /// The `redirect_uri` the code was asked for with.
        redirect: String,
    ) -> Result<Account> {
        Identity::github(&code, &redirect)?.link()
    }

    /// Adds the Google identity behind an authorization code to the calling
    /// user's account, and answers the account.
    #[public]
    pub fn link_google(
        /// The `code` Google sent the user back with.
        code: String,
        /// The `redirect_uri` the code was asked for with.
        redirect: String,
        /// The PKCE `code_verifier`, when the code was asked for with a
        /// challenge.
        verifier: Option<String>,
    ) -> Result<Account> {
        Identity::google(&code, &redirect, verifier.as_deref())?.link()
    }

    /// The calling user's account and the identities that sign in to it.
    #[query]
    #[public]
    pub fn account() -> Result<Account> {
        Account::load(Account::signed_in()?)
    }
}

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
