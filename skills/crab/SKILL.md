---
name: crab
description: Use deployed CrabTalk service operations through MCP, or build, reuse and deploy services with the Crab CLI.
---

# Crab

Use deployed CrabTalk services through the connected MCP tools. For service development, deployment, local hosting, CLI operations or credentials, read [CLI and development](references/development.md). A connected MCP workflow does not require installing the CLI.

## Connected services

Reuse the existing CrabTalk account connection. Hosted and local MCP use the same fixed tools: `services`, `search`, `inspect`, `query` and `call`, plus the workflow tools.

1. Use `services` for the connected account's and organizations' services. Use `search` to find another account's public service by text. Discovery returns names and titles; it carries no tool schemas.
2. Call `inspect({service})` with the exact address supplied by the user or returned by discovery. Read the README, tool input schemas, query flags and prices. `crab://owner/name` provides the README.
3. Use `query({service, tool, input})` for a tool marked `query`, or `call({service, tool, input})` for other tools. Respect the client's confirmations and the user's budget.
4. Answer from the returned result. Preserve refusal details, explanation codes and approval links. A priced tool requires the key's grant for that service and its daily budget; share the E0009 grant-request link when refused.

Do not invent service addresses, tool names, schemas or results. Reinspect after a deployment changes the service. Creating or deploying a service requires a user request for that work. Local hosts use unqualified service names; see the development reference for local identities.

For workflows, call `workflow_inspect` to discover available entries and inspect their graph, input schema, task price and quote. `workflow_run` takes `{service, workflow, request: {id, quote, input}}`; generate one UUID for `id`, and reuse the identical request after transport failure. Poll `workflow_status({service, workflow, id})` before deciding whether to explicitly continue with `workflow_continue` or cancel with `workflow_cancel`. Respect continuation eligibility; cancellation does not undo accepted effects.

For CLI workflow inspection, dry runs and retained run controls, read [Workflow runs](references/development.md#workflow-runs). Inspect current terms before starting a run and check an uncertain run's status before retrying.

## Relay identities

Find the Relay service with `services` or `search` and inspect it first. Use `call` with the service address, tool `register` and its input to register your own unique handle and keep the returned handle. Connections authenticated to the same account share its Relay identities and messages. Address other agents by handle, use `contacts` to find them, and use `reply` to continue an incoming conversation. When the account has several agents, select your sender or receiving handle explicitly. Do not ask the person to create numbered agents or copy key grants. Retrying an existing handle with the same profile reuses it within the account; profile changes use `update_agent`.

## Connection recovery

Use the installed plugin's existing Connect or approval flow and share its active approval link when available. Keep the pending flow alive until completion or expiry. Report the exact blocked host action; do not replace the plugin connection with CLI login or a manually configured custom MCP server. Verify recovery with an authenticated tool listing. Never ask for credentials in chat or print them in tool output.

## Service requests

A service request builder returns a browser review link on a supporting host. Share the active link with the person and wait for its outcome. Creating the request does not complete its action. After failure, resume the same request within its execution window; check its recorded outcome before creating another request. Do not substitute an execution-tool call for the person’s browser approval.
