---
name: crab
description: Find and call deployed CrabTalk tools, start or track workflow runs, and use Relay identities through MCP or the Crab CLI.
---

# Crab

Use deployed CrabTalk services through the connected MCP tools. To call services from a terminal, read [Calling from the CLI](references/cli.md). A connected MCP workflow does not require installing the CLI. Other jobs have their own skills: `crab-build` writes a service, `crab-wrap` turns an HTTP API into one, `crab-ship` deploys and operates it and hands a person what only they can approve, `crab-debug` investigates a failed call.

## Connected services

Reuse the existing CrabTalk account connection. Hosted and local MCP use the same fixed tools: `status`, `services`, `search`, `inspect`, `query` and `call`, plus the workflow tools. Use `status({})` to inspect the connected credential’s access; it requires no account-wide key permission.

1. Use `services` for the connected account's and organizations' services. Use `search` to find another account's public service by text. Discovery returns names and titles; it carries no tool schemas.
2. Call `inspect({service})` with the exact address supplied by the user or returned by discovery. Read the README, tool input schemas, query flags and prices. `crab://owner/name` provides the README.
3. Use `query({service, tool, input})` for a tool marked `query`, or `call({service, tool, input})` for other tools. Respect the client's confirmations and the user's budget.
4. Answer from the returned result. Preserve refusal details, explanation codes and approval links. A priced tool from another account requires the key's service grant and available daily budget. For an E0009 grant or a request for more budget, use `crab-ship` or the [API-key guide](https://crabtalk.ai/docs/guides/keys).

Do not invent service addresses, tool names, schemas or results. Reinspect after a deployment changes the service. Creating or deploying a service requires a user request for that work. Local hosts use unqualified service names; see [Local callers](references/cli.md#local-callers-and-mcp) for local identities.

For workflows, call `workflow_inspect` to discover available entries and inspect their graph, input schema, task price and quote. `workflow_run` takes `{service, workflow, request: {id, quote, input}}`; generate one UUID for `id`, and reuse the identical request after transport failure. Poll `workflow_status({service, workflow, id})` before deciding whether to explicitly continue with `workflow_continue` or cancel with `workflow_cancel`. Respect continuation eligibility; cancellation does not undo accepted effects.

For CLI workflow inspection, dry runs and retained run controls, read [Workflow runs](references/cli.md#workflow-runs). Inspect current terms before starting a run and check an uncertain run's status before retrying.

## Relay identities

To message other agents through Relay, find the Relay service with `services` or `search` and inspect it: its README says how to register a handle, choose a sender and recover delivery.

## Connection recovery

Use the installed plugin's Connect controls to recover its MCP connection. Report the exact blocked host action and verify recovery with an authenticated tool listing. See [MCP connection problems](https://crabtalk.ai/docs/guides/mcp#connection-problems).

## Service requests

If a tool returns a service request, use `crab-ship` or the [service request guide](https://crabtalk.ai/docs/guides/service-requests) for approval and recovery.
