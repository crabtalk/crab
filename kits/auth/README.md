# Auth

Sign-in for a product's users: a GitHub or Google login turned into an ID token the program issues, for an account each of the user's identities signs in to.

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
    use crab_auth::{Account, Identity, Signed};

    /// Signs in with a GitHub authorization code.
    #[public]
    #[anonymous]
    pub fn sign_in(code: String, redirect: String) -> Result<Signed> {
        Identity::github(&code, &redirect)?.sign_in()
    }

    /// The calling user's account.
    #[query]
    #[public]
    pub fn account() -> Result<Account> {
        Account::load(Account::signed_in()?)
    }
}
```

The program declares nothing the kit already declares: its secrets, `berm.token.sign`, the hosts it fetches, and its migrations reach the program's manifest through `uses`. Its tables are `auth_accounts` and `auth_identities`, in the program's database.

- `Identity::github(code, redirect)` and `Identity::google(code, redirect, verifier)`: the identity an authorization code was granted for, `redirect` the `redirect_uri` it was asked for with and `verifier` the PKCE `code_verifier` when it was asked for with a challenge.
- `identity.sign_in()`: a token for the account the identity signs in to, made on its first sign-in. Answers `Signed {token, expires}`.
- `identity.link()`: adds the identity to the calling user's account, and answers the account. Refuses an identity that already signs in to another account.
- `Account::signed_in()`: the id of the calling user's account, when this program issued their token.
- `Account::load(id)`: the account and its identities.

A token is a JWT signed `EdDSA` by the host, valid for an hour: `sub` the account's id, `aud` the program's issuer, and the identity's `name`, `email` and `picture`. `email` is GitHub's public email, or Google's when it is verified.

## As a service

`bin/` is the kit as a service of its own, `auth`: deploy it under your own account, with its own OAuth clients. A product's site sends the user to GitHub or Google, takes the `code` they come back with, and calls `github` or `google` with it, with no token. The token it answers is what the site's later calls carry, through an app that lists this service's issuer with the issuer itself as audience.

- `github(code, redirect)`: signs in with a GitHub authorization code. Answers `{token, expires}`.
- `google(code, redirect, verifier)`: the same for Google.
- `link_github(code, redirect)` and `link_google(code, redirect, verifier)`: add that identity to the calling user's account. Answers the account.
- `account()`: the calling user's account, `{id, identities}`.

## Setting it up

1. Create a GitHub OAuth app whose callback URL is the page on the product's site that takes the code. Set its client ID and a client secret as the secrets `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET`, granted for `github.com`.
2. Create a Google OAuth client of type Web application, with that page among its authorized redirect URIs, asking for the scopes `openid email profile`. Set its client ID and secret as `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`, granted for `oauth2.googleapis.com`.
3. Give the product's app the program's issuer, with the issuer as its audience.

The site checks the `state` it sent the user away with before calling.
