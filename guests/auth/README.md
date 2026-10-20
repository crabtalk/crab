# Auth

Sign-in for a product's users: a GitHub or Google login turned into an ID token this service issues, for an account each of the user's identities signs in to.

A product deploys this service under its own account, with its own OAuth clients. Its site sends the user to GitHub or Google, takes the `code` they come back with, and calls `github` or `google` with it, with no token. The token it answers is what the site's later calls carry, through an app that lists this service's issuer with the issuer itself as audience.

## Tools

- `github(code, redirect)`: signs in with a GitHub authorization code, `redirect` the `redirect_uri` it was asked for with. Answers `{token, expires}`.
- `google(code, redirect, verifier)`: the same for Google, `verifier` the PKCE `code_verifier` when the code was asked for with a challenge.
- `link_github(code, redirect)` and `link_google(code, redirect, verifier)`: add that identity to the calling user's account. Answers the account.
- `account()`: the calling user's account, `{id, identities}`.

An identity's first sign-in makes a new account. `link_*` takes a user this service signed in, and refuses an identity that already signs in to another account.

A token is a JWT signed `EdDSA` by the host, valid for an hour: `sub` the account's id, `aud` this service's issuer, and the identity's `name`, `email` and `picture`. `email` is GitHub's public email, or Google's when it is verified.

## Setting it up

1. Create a GitHub OAuth app whose callback URL is the page on the product's site that takes the code. Set its client ID and a client secret as the secrets `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET`, granted for `github.com`.
2. Create a Google OAuth client of type Web application, with that page among its authorized redirect URIs, asking for the scopes `openid email profile`. Set its client ID and secret as `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`, granted for `oauth2.googleapis.com`.
3. Give the product's app this service's issuer, with the issuer as its audience.

The site checks the `state` it sent the user away with before calling.
