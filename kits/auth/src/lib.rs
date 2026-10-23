//! Sign-in as a kit: a GitHub or Google login turned into a session of the
//! program using it, for an account each of the user's identities signs in
//! to. Its tables are `auth_accounts`, `auth_identities` and `auth_sessions`.

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use berm_lang::Schema;

mod account;
mod provider;

/// A signed-in session.
#[derive(Schema)]
pub struct Session {
    /// What a call carries to act as the account: 64 hex digits.
    pub token: String,
    /// When it expires, in seconds since the Unix epoch.
    pub expires: u64,
}

/// An account, the identities that sign in to it.
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
    #[msg("that session is not signed in")]
    NotSignedIn,
    #[msg("that identity signs in to another account")]
    Taken,
}
