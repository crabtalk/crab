//! Accounts, the identities that sign in to them, and the tokens they are
//! issued.

use crate::{Account, AuthError, Identity, Signed};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{Result, rows, serde_json::json, token};

/// How long a token lasts, in seconds.
const LIFETIME: u64 = 60 * 60;

impl Identity {
    /// A token for the account this identity signs in to, made on its first
    /// sign-in.
    pub fn sign_in(&self) -> Result<Signed> {
        let account = match self.account()? {
            Some(account) => account,
            None => rows::<(i64,)>(
                "INSERT INTO accounts (created) VALUES (?) RETURNING id",
                &(berm_lang::now() / 1000,),
            )?
            .into_iter()
            .next()
            .map(|(id,)| id)
            .ok_or("the database made no account")?,
        };
        self.keep(account)?;
        let issuer = token::issuer()?;
        let expires = berm_lang::now() / 1000 + LIFETIME;
        let token = token::sign(&json!({
            "sub": account.to_string(),
            "aud": issuer,
            "exp": expires,
            "name": self.name,
            "email": self.email,
            "picture": self.avatar,
        }))?;
        Ok(Signed { token, expires })
    }

    /// The calling user's account, after adding this identity to it.
    pub fn link(&self) -> Result<Account> {
        let account = Account::signed_in()?;
        if self.account()?.is_some_and(|found| found != account) {
            return Err(AuthError::Taken.into());
        }
        self.keep(account)?;
        Account::load(account)
    }

    /// The account this identity signs in to.
    fn account(&self) -> Result<Option<i64>> {
        let found = rows::<(i64,)>(
            "SELECT account FROM identities WHERE provider = ? AND subject = ?",
            &(&self.provider, &self.subject),
        )?;
        Ok(found.into_iter().next().map(|(account,)| account))
    }

    /// This identity as signing in to `account`, with its profile as last
    /// answered.
    fn keep(&self, account: i64) -> Result<()> {
        rows::<()>(
            "INSERT INTO identities (provider, subject, account, name, email, avatar)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (provider, subject) DO UPDATE
             SET name = excluded.name, email = excluded.email, avatar = excluded.avatar",
            &(
                &self.provider,
                &self.subject,
                account,
                &self.name,
                &self.email,
                &self.avatar,
            ),
        )?;
        Ok(())
    }
}

impl Account {
    /// The id of the account of the user the call carries, when this service
    /// issued their token.
    pub fn signed_in() -> Result<i64> {
        let user = berm_lang::user()?.ok_or(AuthError::NotSignedIn)?;
        if user.issuer != token::issuer()? {
            return Err(AuthError::NotSignedIn.into());
        }
        Ok(user.subject.parse().map_err(|_| AuthError::NotSignedIn)?)
    }

    /// Account `id` and its identities.
    pub fn load(id: i64) -> Result<Self> {
        let identities = rows::<(
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )>(
            "SELECT provider, subject, name, email, avatar FROM identities
             WHERE account = ? ORDER BY provider, subject",
            &(id,),
        )?;
        Ok(Self {
            id: id.to_string(),
            identities: identities
                .into_iter()
                .map(|(provider, subject, name, email, avatar)| Identity {
                    provider,
                    subject,
                    name,
                    email,
                    avatar,
                })
                .collect::<Vec<_>>(),
        })
    }
}
