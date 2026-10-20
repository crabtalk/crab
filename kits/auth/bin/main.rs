//! Sign-in as a service: the `crab-auth` kit's identities and accounts as
//! tools.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

#[berm_lang::program(uses = [crab_auth], readme = "README.md")]
mod tools {
    use alloc::string::String;
    use berm_lang::Result;
    use crab_auth::{Account, Identity, Signed};

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
