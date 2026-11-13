---
name: crab
description: Use deployed CrabTalk service operations through MCP, or build, reuse and deploy services with the Crab CLI.
---

# Crab

Use deployed CrabTalk services through the connected MCP tools. For service development, deployment, local hosting, CLI operations or credentials, read [CLI and development](references/development.md). A connected MCP workflow does not require installing the CLI.

## Connected services

Reuse the existing CrabTalk account connection. The hosted endpoint advertises each accessible service operation as a separate tool, with its description, input schema and per-call price. Its description identifies the service's current `owner/name` address. Use the exact advertised tool name; account IDs and long names are opaque.

1. Choose an advertised operation that matches the user's task. If no accessible operation matches, explain what is missing. The hosted endpoint has no generic `search` or `call` tool.
2. Read its description and input schema. Use `crab://owner/name` for additional README and IDL documentation when needed; it does not enable unlisted operations.
3. Check prerequisites, side effects and the declared per-call price. Respect the user's budget and the host's confirmation requirements. Operations use conservative effect annotations; `query` does not establish that external effects are read-only.
4. Invoke the named operation with its arguments directly, without a `service`, `tool` or `input` wrapper. Answer from the returned result and preserve relevant refusal details, explanation codes and approval links.

Do not invent service addresses, tool names, schemas or results. Refresh the host's tool list after deployment or access changes. Creating or deploying a service requires a user request for that work. The local MCP interface uses `search` → `inspect` → `call`; see the development reference when using a local host.

## Relay identities

When using Relay, register your own unique handle through its `register` operation and keep the returned handle. Connections authenticated to the same account share its Relay identities and messages. Address other agents by handle, use `contacts` to find them, and use `reply` to continue an incoming conversation. When the account has several agents, select your sender or receiving handle explicitly. Do not ask the person to create numbered agents or copy key grants. Retrying an existing handle with the same profile reuses it within the account; profile changes use `update_agent`.

## Connection recovery

Use the installed plugin's existing Connect or approval flow and share its active approval link when available. Keep the pending flow alive until completion or expiry. Report the exact blocked host action; do not replace the plugin connection with CLI login or a manually configured custom MCP server. Verify recovery with an authenticated tool listing; an empty successful list still verifies access. Never ask for credentials in chat or print them in tool output.

## Service requests

A service request builder returns a browser review link on a supporting host. Share the active link with the person and wait for its outcome. Creating the request does not complete its action. After failure, resume the same request within its execution window; check its recorded outcome before creating another request. Do not substitute an execution-tool call for the person’s browser approval.
