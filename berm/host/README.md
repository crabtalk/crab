# berm-host

The shared host for Berm daemons, built on the Wasmtime-backed [`berm`](../berm) runtime.

`Host<E>` manages deployed services, persistent state, logs, crons and wakes, per-service databases, a log of each call, and usage accounting. The daemon authenticates callers and serves its own routes.

Implement `Env` to supply HTTP transport, image fetching, peer calls, secret resolution, an invocation observer, and the clock. Pass those implementations, an optional proxy, and the layers put around every call from outside as `Edges` to `Host::open`, use `Edges::enter` for an optional scope around every invocation (including nested and scheduled calls), then call `Host::boot` inside a Tokio runtime to restore services and start background work.

The `test` feature adds `berm_host::test`: a host whose edges a test holds, running a service's built image under the real engine. Fetches answer what the test arranged, the clock stands where the test set it, and the test fires the wakes the service armed.

`Config` controls instance capacity, the most and the default budget of CPU, rows read, rows written and memory, call depth, the compiled-code cache, and the database directory. See [`crab dev`](../../crates/crab) for a daemon using this host.

`host::Caller.key` carries the authenticated API key ID. The embedder verifies it; the local host trusts supplied caller metadata. `berm_lang::key::id()` reads that ID throughout the chain, including peer links. Scheduled calls carry no key.

Nested calls use qualified `owner/service` addresses or bare names within the caller's scope. `Peers::call` authorizes calls crossing scopes, including colocated targets; `Peers::link` transports calls within a scope to another host. Local hosts have no account gateway.

`Host::event_page` reads a named `#[tool(events = "name")]` source under the supplied caller's authorization and checks its page framing before returning a resume position. A read without a cursor returns the current checkpoint. Tool authorization refusals return `403`; ordinary source failures return `400`. Native discovery metadata comes from `berm_api::events::Source`. MCP subscription management and callback delivery are not implemented.

`Host::live` returns a fresh `database` snapshot: committed SQLite allocation, the writer's enforced capacity, and an epoch-second measurement time. WAL, logs and code caches are excluded. Recorded database snapshots refresh on deployment and each background flush; history reads do not write samples. The legacy `stored` field remains separate.

## License

AGPL-3.0-only

An invocation scope can refuse admission with `berm_host::Denied`; outside calls keep that HTTP status. HTTP transports receive the sending invocation’s `chain` and `span` on each request.

A service declaring `secret_resources = true` can use `{secret:ID}` for a runtime secret resource. `Secrets::grant` must authorize the complete deployment, resource ID and destination host. Missing grants refuse the request before dispatch; HTTP permissions remain independently enforced. Individual resource IDs are not added to `Deps.secrets`.

`Secrets::resource_owner` supplies account ownership metadata for runtime-resource binding checks; its default returns no grant. `berm.secret.resource` requires the resource capability and HTTP permission. `berm.http.fetch_all.literal_body` keeps body bytes literal while applying the usual header secret grants.
