# berm-host

The shared host for Berm daemons, built on the Wasmtime-backed [`berm`](../berm) runtime.

`Host<E>` manages deployed services, persistent state, logs, crons and wakes, per-service databases, a log of each call, and usage accounting. The daemon authenticates callers and serves its own routes.

Implement `Env` to supply HTTP transport, image fetching, peer calls, secret resolution, an invocation observer, and the clock. Pass those implementations and the layers put around every call from outside as `Edges` to `Host::open`, use `Edges::enter` for an optional scope around every invocation (including nested and scheduled calls), then call `Host::boot` inside a Tokio runtime to restore services and start background work.

The `test` feature adds `berm_host::test`: a host whose edges a test holds, running a service's built image under the real engine. Fetches answer what the test arranged, the clock stands where the test set it, and the test fires the wakes the service armed.

`Config` controls instance capacity, the most and the default budget of CPU, rows read, rows written and memory, call depth, executable residency, the disk compilation cache, and the database directory. See [`crab dev`](../../crates/crab) for a daemon using this host.

`code_cache` defaults to 64 MiB of accounted image bytes and `code_idle` to `"5m"`. Idle code is evicted without stopping or removing its deployment. The next outside, nested, scheduled or peer call reloads it. `cache_dir` defaults to `cache` beside the host state file and retains Wasmtime artifacts across restarts. Boot restores metadata without compiling code; older rows without a footprint compile once, record it and discard the executable. Restoration keeps existing deployments discoverable when their account allowance has been lowered. Metadata reads and storage sampling include cold services without warming them. Cache expiry does not change execution memory-time charges.

`host::Caller.key` carries the authenticated API key ID. The embedder verifies it; the local host trusts supplied caller metadata. `berm_lang::key::id()` reads that ID throughout the chain, including peer links. Scheduled calls carry no key.

Nested calls use qualified `owner/service` addresses or bare names within the caller's scope. `Peers::call` authorizes calls crossing scopes, including colocated targets; `Peers::link` transports calls within a scope to another host. Local hosts have no account gateway. Local caller storage selects the unchecked caller and any supplied user. Removing a deployment retains its caller files.

For a group deploy, the host preflights every member, switches members one at a time and attempts to restore earlier images if a later switch fails. Applied migrations stay in place and may prevent restoration.

`Host::event_page` reads a named `#[tool(events = "name")]` source under the supplied caller's authorization and checks its page framing before returning a resume position. A read without a cursor returns the current checkpoint. Tool authorization refusals return `403`; ordinary source failures return `400`. Native discovery metadata comes from `berm_api::events::Source`. MCP subscription management and callback delivery are not implemented.

`Host::live` returns a fresh `database` snapshot: committed SQLite allocation, the writer's enforced capacity, and an epoch-second measurement time. WAL, logs and code caches are excluded. Recorded database samples refresh on deployment and during minute maintenance. History reads use those samples. The host persists local calls, logs, usage, and meters every second or when 512 calls have accumulated. The legacy `stored` field remains separate.

Outside calls denied a node call slot or a host memory reservation receive HTTP 503 with E0098.

Service databases share caches of at most 64 idle writer connections and 64 idle read connections per host. Active transactions retain their writer connection. Writer jobs execute in submission order on Tokio's blocking pool; idle services hold no dedicated writer thread. Capacity limits and SQL guards are reapplied when a connection reopens.

Replay tapes hash HTTP requests before substituting credentials and retain responses as received. An upstream response can put a credential on a tape.

## License

AGPL-3.0-only

An invocation scope can refuse admission with `berm_host::Denied`; outside calls keep that HTTP status. HTTP transports receive the sending invocation’s `chain` and `span` on each request.

A service declaring `secret_resources = true` can use `{secret:ID}` for a runtime secret resource. `Secrets::grant` must authorize the complete deployment, resource ID and destination host. An authenticated call can use granted resources owned by its account. Scheduled calls require an account binding for the resource, deployment and destination, and the grant must still be active. Bindings survive host restarts. Rebind resources after a deployment moves to another host. Missing grants refuse the request before dispatch; HTTP permissions remain independently enforced. Individual resource IDs are not added to `Deps.secrets`.

`Secrets::resource_owner` supplies account ownership metadata for runtime-resource binding checks; its default returns no grant. `berm.secret.resource` and `berm.secret.bind` require the resource capability and HTTP permission. Binding requires an authenticated write call. `berm.http.fetch_all.literal_body` keeps body bytes literal while applying the usual header secret grants.

Workflow declarations load from the service image, including services with no tools. The owner runtime includes control budgets, spend caps and checkpoint policy; public IDL includes only discoverable entries, graphs and task prices. Invalid graph/configuration records are refused before execution with `E0058`. Workflow schedules use five-field UTC cron expressions and copy their configured input into each run. Occurrence identities deduplicate delivery; stopped services and downtime skip occurrences without backfill. Refused and failed runs are not retried automatically. Embedders can admit owner-funded runs through `Peers::scheduled`; the native host admits them locally by default.

`Host::control` executes a registered workflow control function. Application syscalls fail with HTTP status 422 and `E0059`; admission refusals retain their original status, headers and error body. Resource exhaustion, traps and oversized input/output fail the control invocation.

`Host::workflows` keeps complete runs and accepted dependency bindings in a separate durable ledger. Admission takes a storage grant for the owning scope and reserves the run's checkpoint capacity plus each retained image once per scope. Every write checks that capacity; execution data is never shortened. Claim tokens fence checkpoint writes; lease deadlines prevent new dispatch. Expired claims become indeterminate without starting another attempt.

Stopping a service blocks continuation until it starts again. Deleting a service permanently invalidates its retained runs; recreating its name does not restore eligibility. Upgrades leave retained workflow images intact. Expiration or explicit run deletion removes continuation data and releases its reservation. Current authorization and dependency checks belong to the execution/admission boundary; checkpoint availability alone does not authorize continuation.

`prepare_workflow` resolves locally deployed tool bindings without running a guest. `execute_workflow` claims a pending accepted run, executes its retained graph under one deadline, and persists ordered results before dependent nodes. The embedder authorizes admission and grants storage before execution. Cross-account calls use the account gateway; receiver-side binding checks precede dispatch. Targets declaring an idempotency window require a nonempty `idempotency_key` in prepared inputs. The final output and successful run status commit together.

`workflow_offer` reads the source, graph, configuration and local bindings without executing a guest. `workflow_binding` resolves a public service-call target. The corresponding metadata routes require the daemon’s authorization. `prepare_accepted_workflow` retains externally resolved bindings; `execute_claimed_workflow` runs an existing claim. The embedder supplies spend authorization for that exact run, attempt and deadline. Nested links carry it; owner wakes are refused with `E0063`.

Terminal outcomes enter the completion outbox in the same transaction as the final run record. Embedders deliver `pending_completions` and acknowledge each run/attempt after acceptance. Receipts survive run deletion and checkpoint expiry. Expired claims produce indeterminate receipts without restarting execution.

`continue_workflow` manually claims a failed or indeterminate run with a fresh deadline. It preserves the original image, input, prepared payloads, branch choices, successful items and dependency bindings. The embedder must reauthorize the initiator, original funding authority and remaining cumulative spend before executing the claim. `Peers::receipt` can supply a durable result without dispatching the tool again. Unknown writes require the original key and the target's unexpired idempotency guarantee. There are no automatic step retries or crash continuation.

A tool's `#[tool(idempotency = "24h")]` declaration declares a guarantee enforced by that tool or its upstream provider. The host does not add idempotency to arbitrary tools. Prepared inputs must contain `idempotency_key`; continuation never changes the payload, key or original window start.

`cancel_workflow` fences new graph dispatch and drains active calls. It cannot undo their effects. Cancellation committed before the terminal result makes the attempt cancelled; cancellation after committed success preserves that success. The completion outbox reports the resulting terminal status.

`berm_host::router` exposes workflow inspection, asynchronous start, run status, continuation and cancellation below `/workflows/{workflow}`. Start returns `202`; reuse the original UUID, quote and input after a lost response. Consumer responses use `private, no-store` and omit runtime credentials and accounting headers. Local inspection reports an effective task price of zero; the local runtime does not settle task fees.
