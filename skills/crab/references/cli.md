# Calling from the CLI

`crab` talks to the host its context names: `default` is the local host at `127.0.0.1:7777`, a context made by `crab login` is Cloud, and `--context` picks another for one command. `--org` acts for an org. Commands with a JSON output mode use it with `--json` or when stdout is piped.

## Credentials

`crab status --json` reports the selected host and account separately from the authenticated credential. Keys include their permissions, expiry, priced-service grants and daily budget and spend. It requires no management scope. MCP `status({})` returns the same credential view for its own connection; CLI and MCP may use different credentials. For Cloud sign-in, use `crab-ship`.

## Calling another account's service

- On Cloud, a service is addressed as `owner/name`, its page at `https://crabtalk.ai/{owner}/{name}`. `crab inspect owner/name` prints another account's public service: its README and its public tools with their arguments. Read it before the first call.
- `crab call owner/name <tool> '<arguments as JSON>'` calls one of its public tools, reading the arguments from stdin when they are left out. It prints the result, and what the call used on stderr; a tool failure exits with status 2.
- For priced-tool access or a higher key budget, use `crab-ship`.
- For refusal codes and failed calls, use `crab-debug`.

## Workflow runs

- Inspect a deployed contract with `crab workflow inspect owner/service.workflow`; on the local context use `service.workflow`. `workflow dry-run` is an alias for inspection. The graph exposes dependencies and bounded call counts; current terms separate the successful-result task price from the owner's cumulative managed-spend cap. Detailed owner accounting is owner-only.
- `crab workflow run owner/service.workflow '<input as JSON>' --dry-run` validates input without starting a run. `run ... --yes` admits one run and returns its identity; an interactive terminal asks for confirmation without `--yes`. Input may come from stdin. In scripts, stdout is one JSON object and the pre-execution graph and terms print on stderr. Reuse existing `--context` and `--org` selection; Cloud uses the credential and local admission defaults to the unchecked stable caller `local`.
- Track it with `crab workflow status owner/service.workflow <run-id>`. After a lost admission response, use the ID printed on stderr and check its status before any new run. `run --id <run-id>` resends the same request only with identical input and terms. Do not use a new ID to retry an uncertain operation.
- `workflow continue <address> <run-id> --yes` keeps original inputs and completed checkpoints and rechecks eligibility, terms, funding and the remaining cap. `workflow cancel <address> <run-id> --yes` stops new calls; it cannot undo effects already accepted by a target. Respect the user's authorization before either operation. Read expiry, continuation refusals and explanation codes; never repeatedly start new runs to bypass them.

## Local callers and MCP

- `crab --context default call <service> <tool> '<arguments>'` calls a public tool locally as the unchecked caller `local`. Use `--caller alice` to override it, or `--anonymous` to call without a caller; only tools with `access = "anonymous"` accept anonymous calls. These flags are local-only; Cloud uses the credential.
- Local MCP inspection returns caller-filtered tool schemas in `definition` and an empty `prices` map.
- Local MCP uses Streamable HTTP at `http://127.0.0.1:7777/mcp`. Send `x-berm-caller: {"id":"alice"}` to discover and call public tools as that caller. Without the header, tool discovery and calls are anonymous. Local identities are unverified. Keep the endpoint on loopback.
