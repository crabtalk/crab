# Auth

Sign-in for a product's users: a GitHub or Google login turned into a session the program keeps in its own database, for an account each of the user's identities signs in to.

## As a library

Add the kit to your program's crate:

```toml
[dependencies]
crab-auth = "0.1"
```

Name it in `uses`, and call it from your own tools:

```rust,ignore
#[berm_lang::program(uses = [crab_auth])]
mod tools {
    use alloc::string::String;
    use berm_lang::Result;
    use crab_auth::{Account, Identity, Session};

    /// Signs in with a GitHub authorization code.
    #[public]
    #[anonymous]
    pub fn sign_in(code: String, redirect: String) -> Result<Session> {
        Identity::github(&code, &redirect)?.sign_in()
    }

    /// The account a session is signed in to.
    #[query]
    #[public]
    #[anonymous]
    pub fn account(session: String) -> Result<Account> {
        Account::load(Account::session(&session)?)
    }
}
```

The program declares nothing the kit already declares: its secrets, the hosts it fetches, and its migrations reach the program's manifest through `uses`. Its tables are `auth_accounts`, `auth_identities` and `auth_sessions`, in the program's database.

- `Identity::github(code, redirect)` and `Identity::google(code, redirect, verifier)`: the identity an authorization code was granted for, `redirect` the `redirect_uri` it was asked for with and `verifier` the PKCE `code_verifier` when it was asked for with a challenge.
- `identity.sign_in()`: a session of the account the identity signs in to, made on its first sign-in. Answers `Session {token, expires}`.
- `identity.link(session)`: adds the identity to the account `session` is signed in to, and answers the account. Refuses an identity that already signs in to another account.
- `Account::session(token)`: the id of the account the session is signed in to, refused once it expired or was signed out.
- `Account::load(id)`: the account and its identities.
- `Session::end(token)`: signs the session out, answering whether it was signed in.

A session lasts 30 days. Its token is 32 bytes from `berm_lang::random` as 64 hex digits; `auth_sessions` keeps only its sha256. `email` is GitHub's public email, or Google's when it is verified.

## As a service

`bin/` is the kit as a service of its own, `auth`: deploy it under your own account, with its own OAuth clients. A product's site sends the user to GitHub or Google, takes the `code` they come back with, and calls `github` or `google` with it. The token it answers is what the site passes as `session` to the calls after.

- `github(code, redirect)`: signs in with a GitHub authorization code. Answers `{token, expires}`.
- `google(code, redirect, verifier)`: the same for Google.
- `link_github(session, code, redirect)` and `link_google(session, code, redirect, verifier)`: add that identity to the session's account. Answers the account.
- `account(session)`: the session's account, `{id, identities}`.
- `sign_out(session)`: ends the session. Answers whether it was signed in.

## Setting it up

1. Create a GitHub OAuth app whose callback URL is the page on the product's site that takes the code. Set its client ID and a client secret as the secrets `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET`, granted for `github.com`.
2. Create a Google OAuth client of type Web application, with that page among its authorized redirect URIs, asking for the scopes `openid email profile`. Set its client ID and secret as `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`, granted for `oauth2.googleapis.com`.
The site checks the `state` it sent the user away with before calling.
