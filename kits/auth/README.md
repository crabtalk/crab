# Auth

Sign-in for a product's users: a GitHub or Google login turned into a session the program keeps in its own database, for an account each of the user's identities signs in to.

## Using it

Add the kit to your program's crate:

```toml
[dependencies]
crabkit = { version = "0.1", features = ["auth"] }
```

The kit is also the crate `crabkit-auth` on its own.

Call it from your own tools:

```rust,ignore
use alloc::string::String;
use berm_lang::{Result, tool};
use crabkit::auth::{Account, Identity, Session};

/// Signs in with a GitHub authorization code.
#[tool(public, anonymous)]
pub fn sign_in(code: String, redirect: String) -> Result<Session> {
    Identity::github(&code, &redirect)?.sign_in()
}

/// The account a session is signed in to.
#[tool(query, public, anonymous)]
pub fn account(session: String) -> Result<Account> {
    Account::load(Account::session(&session)?)
}
```

The program declares nothing the kit already declares: the kit's `berm.toml` declares its secrets, the hosts it fetches, and its migrations, and they reach the program's manifest when the program links the kit. Its tables are `auth_accounts`, `auth_identities` and `auth_sessions`, in the program's database.

- `Identity::github(code, redirect)` and `Identity::google(code, redirect, verifier)`: the identity an authorization code was granted for, `redirect` the `redirect_uri` it was asked for with and `verifier` the PKCE `code_verifier` when it was asked for with a challenge.
- `identity.sign_in()`: a session of the account the identity signs in to, made on its first sign-in. Answers `Session {token, expires}`.
- `identity.link(session)`: adds the identity to the account `session` is signed in to, and answers the account. Refuses an identity that already signs in to another account.
- `Account::session(token)`: the id of the account the session is signed in to, refused once it expired or was signed out.
- `Account::load(id)`: the account and its identities.
- `Session::end(token)`: signs the session out, answering whether it was signed in.

A session lasts 30 days. Its token is 32 bytes from `berm_lang::random` as 64 hex digits; `auth_sessions` keeps only its sha256. `email` is GitHub's public email, or Google's when it is verified.

## Setting it up

The kit declares its secrets as two credentials, `github` (a GitHub OAuth app) and `google` (a Google OAuth client of type Web application, with the scopes `openid email profile`), each with a guide to getting it. Both send the user back to the page on the product's site that takes the code: `crab secret request <service> --copy callback=<that page's URL>` asks a person to set them.

The site checks the `state` it sent the user away with before calling.
