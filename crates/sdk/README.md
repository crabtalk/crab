# crabtalk-sdk

Rust types and HTTP client for the CrabTalk Cloud API at `https://api.crabtalk.ai`. The same client also talks to the local host, `crab dev`.

## Install

```toml
[dependencies]
crabtalk-sdk = { version = "0.1", features = ["client"] }
```

| Feature   | Adds                                              |
| --------- | ------------------------------------------------- |
| `client`  | `crabtalk_sdk::client`, the HTTP client (reqwest) |
| `utoipa`  | `utoipa::ToSchema` on every wire type             |

With no features, the crate provides request and response types and display formatting, without the HTTP client.

## Usage

```rust
use crabtalk_sdk::client::{Client, Error, Owner};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::new("https://api.crabtalk.ai")
        .token(std::env::var("CRABTALK_TOKEN").expect("CRABTALK_TOKEN"))
        .owner(Owner::User("alice".into()));

    for service in client.list().await? {
        println!("{}", service.name);
    }
    Ok(())
}
```

The token is a session token or an API key. Without `owner`, the client addresses a local host at `base`, and account calls return `Error::Host`.

`Client::health` probes `/meta/health` on Cloud and the local host. `Client::plans` requires a Cloud owner; local clients return `Error::Host` with login guidance.

`Client::deployment_status` returns pending approvals and recorded deploy attempts, including before a service exists. `Client::deploys` returns recorded attempts only.

`Client::secret_resources` lists secret metadata and service grants; Cloud API keys need `secrets:read`. Creating, rotating and deleting resources, and changing grants, require a session. Values are never returned. `Client::ask_secret_resource` creates an approval request for a person to supply a value.

`Client::update_key` applies budget, scope and service access together. `UpdateKeyRequest::services` replaces the granted `owner/name` list; `Some(vec![])` clears it, and `None` preserves it. Updating keys requires a session.

A `429` that names its `Retry-After` is sent again after that wait, up to three times, unless its body streams. `Error::retryable` is true for transport failures, `429` and `5xx` only. A structured refusal carries its stable `code` (e.g. `orgs_closed`); branch on that rather than on the status.

Billing balances keep purchased funds in `Credit::micros` and trial funds in `Credit::trial`, with grant amount, reserved funds and expiry. `Credit::recovery_until` records the account's hosting recovery deadline, including accounts that never received trial credit. `PublicPlan::trial` and `Verification::trial` expose the verification offer. `CreditHistory` reports purchased and trial activity separately.

Workflow commercial contracts live in `services::workflow`. `Terms` keeps the fixed customer task price, conservative direct-call quote, and cumulative owner cap separate, with accepted execution bindings, an immutable rate snapshot and revision, and independently billed providers. `Spend` retains incurred and reserved USD micros as exact decimal strings; `Settlement` carries reservation or transaction identities. Execution contracts are in `berm_api::workflow`. Consumer inspection and run responses omit private owner accounting unless the authenticated caller manages the service.

Deployment responses and template manifests retain workflow declarations from the image. Public `berm_api::Service.workflows` contains entry/graph, access, title and task price; owner `berm_api::Runtime.workflows` includes the full configuration. Older service responses may omit these fields.

## Workflow runs

`Client::workflow(service, workflow, caller)` inspects the entry without executing it. `Client::start_workflow(service, workflow, &Start { id, quote, input }, caller)` returns the admitted run with its accepted terms. Generate a UUID for each new run; reuse the same identity, quote and input when retrying an unanswered request. Transport retries never start another attempt. On Cloud, pass `None` for `caller`; a local host accepts an explicitly supplied, unverified `berm_api::host::Caller`.

`Client::workflow_run(service, workflow, run)` reads retained outcomes. `continue_workflow` and `cancel_workflow` take the same arguments. Continuation is manual and preserves completed checkpoints, original payloads and operation keys. Cancellation stops new dispatches, drains in-flight work and does not undo effects. Only the owner and original authenticated initiator can read or manage a Cloud run. The SDK's optional accounting fields contain owner-only terms, incurred/reserved spend and customer settlement.

## Modules

| Module    | Holds                                               |
| --------- | --------------------------------------------------- |
| `account` | Auth, users, orgs and teams, API keys, apps, billing |
| `services` | Services: deploys, secrets, meters, costs |
| `system`  | `/meta` (version, rates) and feedback               |
| `mcp` | Shared tool names, workflow actions and argument schemas, discovery and inspection types |
| `error`   | The two error envelopes the API answers with        |
| `format`  | Byte counts in B, KB, MB, GB and TB (powers of 1024) |
| `client`  | `Client`, `Owner`, `Error` (feature `client`)        |

`berm_api` is re-exported for the service, call and meter types the service calls return.

## License

Apache-2.0

Service usage includes an optional `database` snapshot with `used`, `limit` and epoch-second `at`. Missing snapshots from older hosts are unknown. Account periods expose `storage.databases` and `storage.objects`, the two components of allocated storage.

Billing entries distinguish tool and workflow sales with `price::Kind`. Workflow settlement separates owner and unpriced runs from held, released and settled customer prices. A successful run has one settled customer charge across all attempts.

`Earned::net` reports profit after all attributed costs. `Earned::payable` excludes costs already charged to the owning account; payout calculations use this balance before transfer fees.

`Client::auth_status` reads `GET /auth/status` for the current credential without a management scope. Its `account::auth::Status` distinguishes API keys, sessions and local callers; API keys include their account, permissions, expiry, daily budget and spend, and priced-service grants. The MCP `status` tool returns the same credential view.
