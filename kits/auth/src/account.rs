//! Accounts, the identities that sign in to them, and their sessions.

use crate::{Account, AuthError, Identity, Session};
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{Result, rows};
use sha2::{Digest, Sha256};

/// How long a session lasts, in seconds.
const LIFETIME: u64 = 30 * 24 * 60 * 60;

impl Identity {
    /// A session of the account this identity signs in to, made on its first
    /// sign-in.
    pub fn sign_in(&self) -> Result<Session> {
        let account = match self.account()? {
            Some(account) => account,
            None => rows::<(i64,)>(
                "INSERT INTO auth_accounts (created) VALUES (?) RETURNING id",
                &(berm_lang::now() / 1000,),
            )?
            .into_iter()
            .next()
            .map(|(id,)| id)
            .ok_or("the database made no account")?,
        };
        self.keep(account)?;
        Session::open(account)
    }

    /// The account `session` is signed in to, after adding this identity to
    /// it.
    pub fn link(&self, session: &str) -> Result<Account> {
        let account = Account::session(session)?;
        if self.account()?.is_some_and(|found| found != account) {
            return Err(AuthError::Taken.into());
        }
        self.keep(account)?;
        Account::load(account)
    }

    /// The account this identity signs in to.
    fn account(&self) -> Result<Option<i64>> {
        let found = rows::<(i64,)>(
            "SELECT account FROM auth_identities WHERE provider = ? AND subject = ?",
            &(&self.provider, &self.subject),
        )?;
        Ok(found.into_iter().next().map(|(account,)| account))
    }

    /// This identity as signing in to `account`, with its profile as last
    /// answered.
    fn keep(&self, account: i64) -> Result<()> {
        rows::<()>(
            "INSERT INTO auth_identities (provider, subject, account, name, email, avatar)
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

impl Session {
    /// A new session of `account`, its token never kept. Clears every
    /// session that has expired.
    fn open(account: i64) -> Result<Self> {
        let now = berm_lang::now() / 1000;
        rows::<()>("DELETE FROM auth_sessions WHERE expires <= ?", &(now,))?;
        let mut bytes = [0; 32];
        berm_lang::random(&mut bytes);
        let token = hex(&bytes);
        let expires = now + LIFETIME;
        rows::<()>(
            "INSERT INTO auth_sessions (hash, account, expires) VALUES (?, ?, ?)",
            &(hash(&token), account, expires),
        )?;
        Ok(Self { token, expires })
    }

    /// Sign `token`'s session out. Answers whether it was signed in.
    pub fn end(token: &str) -> Result<bool> {
        let ended = rows::<(i64,)>(
            "DELETE FROM auth_sessions WHERE hash = ? RETURNING account",
            &(hash(token),),
        )?;
        Ok(!ended.is_empty())
    }
}

impl Account {
    /// The id of the account `token`'s session is signed in to, refused once
    /// it expired or was signed out.
    pub fn session(token: &str) -> Result<i64> {
        let found = rows::<(i64,)>(
            "SELECT account FROM auth_sessions WHERE hash = ? AND expires > ?",
            &(hash(token), berm_lang::now() / 1000),
        )?;
        let (account,) = found.into_iter().next().ok_or(AuthError::NotSignedIn)?;
        Ok(account)
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
            "SELECT provider, subject, name, email, avatar FROM auth_identities
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

/// `token`'s sha256, as `auth_sessions` keys it.
fn hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// `bytes` as lowercase hex.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
