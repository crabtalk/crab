//! Sign-in as a service: the `crab-auth` kit's identities and accounts as
//! tools.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

#[berm_lang::program(uses = [crab_auth], readme = "README.md")]
mod tools {
    use alloc::string::String;
    use berm_lang::Result;
    use crab_auth::{Account, Identity, Session};

    /// Signs in with a GitHub authorization code, creating the account on the
    /// identity's first sign-in, and answers a session of it.
    #[public]
    #[anonymous]
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
    ) -> Result<Session> {
        Identity::google(&code, &redirect, verifier.as_deref())?.sign_in()
    }

    /// Adds the GitHub identity behind an authorization code to the account
    /// a session is signed in to, and answers the account.
    #[public]
    #[anonymous]
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
    #[public]
    #[anonymous]
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
    #[query]
    #[public]
    #[anonymous]
    pub fn account(
        /// The session's token.
        session: String,
    ) -> Result<Account> {
        Account::load(Account::session(&session)?)
    }

    /// Signs a session out, answering whether it was signed in.
    #[public]
    #[anonymous]
    pub fn sign_out(
        /// The session's token.
        session: String,
    ) -> Result<bool> {
        Session::end(&session)
    }
}
