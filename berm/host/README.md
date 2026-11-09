# berm-host

The shared host for Berm daemons, built on the Wasmtime-backed [`berm`](../berm) runtime.

`Host<E>` manages deployed services, persistent state, logs, crons and wakes, per-service databases, a log of each call, and usage accounting. The daemon authenticates callers and serves its own routes.

Implement `Env` to supply HTTP transport, image fetching, peer calls, secret resolution, an invocation observer, and the clock. Pass those implementations, an optional proxy, and the layers put around every call from outside as `Edges` to `Host::open`, use `Edges::enter` for an optional scope around every invocation (including nested and scheduled calls), then call `Host::boot` inside a Tokio runtime to restore services and start background work.

The `test` feature adds `berm_host::test`: a host whose edges a test holds, running a service's built image under the real engine. Fetches answer what the test arranged, the clock stands where the test set it, and the test fires the wakes the service armed.

`Config` controls instance capacity, the most and the default budget of CPU, rows read, rows written and memory, call depth, the compiled-code cache, and the database directory. See [`crab dev`](../../crates/crab) for a daemon using this host.

Nested calls use qualified `owner/service` addresses or bare names within the caller's scope. `Peers::call` authorizes calls crossing scopes, including colocated targets; `Peers::link` transports calls within a scope to another host. Local hosts have no account gateway.

## License

AGPL-3.0-only

An invocation scope can refuse admission with `berm_host::Denied`; outside calls keep that HTTP status. HTTP transports receive the sending invocation’s `chain` and `span` on each request.
