//! The reference sign-in service: GitHub and Google sign-in through
//! `crabkit-auth`, each identity's account and its sessions as tools.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use auth::{Account, Identity, Session};
use berm_lang::{Result, tool};

/// Signs in with a GitHub authorization code, creating the account on the
/// identity's first sign-in, and answers a session of it.
#[tool]
pub fn github(
    /// The `code` GitHub sent the user back with.
    code: String,
    /// The `redirect_uri` the code was asked for with.
    redirect: String,
) -> Result<Session> {
    Identity::github(&code, &redirect)?.sign_in()
}

/// Signs in with a Google authorization code, creating the account on the
/// identity's first sign-in, and answers a session of it.
#[tool]
pub fn google(
    /// The `code` Google sent the user back with.
    code: String,
    /// The `redirect_uri` the code was asked for with.
    redirect: String,
    /// The PKCE `code_verifier`, when the code was asked for with a
    /// challenge.
    verifier: Option<String>,
) -> Result<Session> {
    Identity::google(&code, &redirect, verifier.as_deref())?.sign_in()
}

/// Adds the GitHub identity behind an authorization code to the account
/// a session is signed in to, and answers the account.
#[tool]
pub fn link_github(
    /// The session's token.
    session: String,
    /// The `code` GitHub sent the user back with.
    code: String,
    /// The `redirect_uri` the code was asked for with.
    redirect: String,
) -> Result<Account> {
    Identity::github(&code, &redirect)?.link(&session)
}

/// Adds the Google identity behind an authorization code to the account
/// a session is signed in to, and answers the account.
#[tool]
pub fn link_google(
    /// The session's token.
    session: String,
    /// The `code` Google sent the user back with.
    code: String,
    /// The `redirect_uri` the code was asked for with.
    redirect: String,
    /// The PKCE `code_verifier`, when the code was asked for with a
    /// challenge.
    verifier: Option<String>,
) -> Result<Account> {
    Identity::google(&code, &redirect, verifier.as_deref())?.link(&session)
}

/// The account a session is signed in to, and the identities that sign
/// in to it.
#[tool(query)]
pub fn account(
    /// The session's token.
    session: String,
) -> Result<Account> {
    Account::load(Account::session(&session)?)
}

/// Signs a session out, answering whether it was signed in.
#[tool]
pub fn sign_out(
    /// The session's token.
    session: String,
) -> Result<bool> {
    Session::end(&session)
}
