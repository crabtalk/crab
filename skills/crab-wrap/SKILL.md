---
name: crab-wrap
description: Turn an existing HTTP provider API into a CrabTalk service. Use for OpenAPI-generated or handwritten adapters, declaring provider credentials and testing against provider responses.
---

# Wrap an HTTP API

A wrapped API is an ordinary service: follow `crab-build` for service reuse, the crate, local loop, budgets, catalog jobs and README. This skill covers what is particular to putting a provider's API behind tools.

## Before writing code

1. Agree the tool list and what it excludes with the service owner when the scope is unclear. Cover the operations and options the caller needs.
2. Read the provider's documentation for each operation in scope. Discovering another endpoint does not add it to scope.

## From an OpenAPI document

Use `crab-openapi` when the provider publishes an OpenAPI 3.0 or 3.1 document. Read its [README](https://github.com/crabtalk/crab/blob/main/crates/openapi/README.md) and [Build from OpenAPI](https://crabtalk.ai/docs/development/openapi) first.

- `crab_openapi::service!` maps tool names to operations (`fetch_video: "GET /api/v1/…"`) and generates the request types, an `api` module and `#[tool]` functions. A generated operation accepts the same `#[tool(...)]` declaration and doc-comment title in its operations map. Entries without a doc comment use the operation summary as the title. A missing or invalid summary requires an entry doc comment. The operation description supplies the tool description.
- Point `spec:` at a local OpenAPI file. When pulling an upstream spec, pin a revision or reviewed digest and review the contract diff before changing it. A build script can prepare that file in `OUT_DIR` and expose it with `cargo::rustc-env=OPENAPI_SPEC=...`, then pass `spec: env!("OPENAPI_SPEC")` to the macro.
- Unsupported shapes (external references, cookie parameters, server templates, other parameter encodings, deprecated operations) are compile errors. Leave those operations out or write them by hand.

## By hand

Use `berm_lang::http::Client` for the provider's shared headers, non-2xx errors and JSON decoding:

```rust
const CLIENT: Client<'static> = Client {
    name: "Resend",
    headers: &[("Authorization", "Bearer {secret:RESEND_API_KEY}")],
};
```

`client.json::<T>(method, url, body)` decodes a successful response; failures carry the provider name, status, body and retry metadata. Keep provider-specific response envelopes and body-level failures (a 200 that reports an error) in the service. See [HTTP](https://crabtalk.ai/docs/development/http).

## Credentials

Every provider secret belongs to a credential in `crab.toml`:

```toml
[credentials.resend]
title = "Resend API key"
guide = "Create an API key at https://resend.com/api-keys for the sending domain you use."
hosts = ["api.resend.com"]
secrets = [{ name = "RESEND_API_KEY", label = "API key" }]
```

- `guide` names the provider, the steps in its dashboard and the URL of the page; the person reads it when they set the secret. `copy` supplies values the person enters at the provider, such as a callback URL. `pattern` refuses a malformed value. `optional = true` lets the service run with it unset. A header referencing an unset optional secret is omitted. A body referencing one is refused.
- `hosts` names, in full, each host its secrets are sent to. Those hosts also allow direct requests.
- For provisioning credentials, use `crab-ship`. Full reference: [Credentials](https://crabtalk.ai/docs/development/credentials).

## Tests

Test each tool against arranged provider answers, never the live API: `test::respond` arranges what an HTTP request answers by method and URL, and `test::fetched` lists the requests sent, with `{secret:NAME}` as written. `berm_host::test::TestHost` runs the built image under the real engine with `arrange` and `grant`. See [Testing](https://crabtalk.ai/docs/development/testing).
