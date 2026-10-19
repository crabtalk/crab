# crabtalk-sdk

Rust types and HTTP client for the CrabTalk Cloud API at `https://api.crab.talk`. The same client also talks to a local bermd.

## Install

```toml
[dependencies]
crabtalk-sdk = { git = "https://github.com/crabtalk/berm", branch = "main", features = ["client"] }
```

| Feature   | Adds                                              |
| --------- | ------------------------------------------------- |
| `client`  | `crabtalk_sdk::client`, the HTTP client (reqwest) |
| `openapi` | `utoipa::ToSchema` on every wire type             |

With no features, the crate is the request and response types, with `serde` only.

## Usage

```rust
use crabtalk_sdk::client::{Client, Error, Owner};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::new("https://api.crab.talk")
        .token(std::env::var("CRABTALK_TOKEN").expect("CRABTALK_TOKEN"))
        .owner(Owner::User("alice".into()));

    for service in client.list().await? {
        println!("{}", service.name);
    }
    Ok(())
}
```

The token is a session token or an API key. Without `owner`, the client addresses a bermd at `base`, and account calls return `Error::Host`.

`Error::retryable` is true for transport failures, `429` and `5xx` only. A structured refusal carries its stable `code` (e.g. `orgs_closed`); branch on that rather than on the status.

## Modules

| Module    | Holds                                               |
| --------- | --------------------------------------------------- |
| `account` | Auth, users, orgs and teams, API keys, apps, billing |
| `sandbox` | Services: deploys, secrets, visibility, meters, costs |
| `system`  | `/meta` (version, rates) and feedback               |
| `error`   | The two error envelopes the API answers with        |
| `client`  | `Client`, `Owner`, `Error` (feature `client`)        |

`berm_api` is re-exported for the program, call and meter types the service calls return.

## License

Apache-2.0
