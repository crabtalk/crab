---
name: use-crabtalk
description: Find deployed CrabTalk services and call their tools when the user asks to use CrabTalk or a service hosted on it.
---

# CrabTalk

Use the connected CrabTalk MCP tools to find and call deployed services as the connected account.

| Tool or resource | Use it for |
| --- | --- |
| `search` | Find deployed services by task, service address, catalog job or category. Results include tool descriptions and prices. |
| `crab://owner/name` | Read a service's README and IDL, including its tool input schemas and prerequisites. This is an MCP resource. |
| `call` | Run a service's tool with `service`, `tool` and `input`. Priced calls charge the connected CrabTalk account. |

## Flow

1. `search` for the task in plain words, or use the exact service address when the user supplies one. Read the returned services and tool descriptions before choosing. An empty result means no matching deployed service was found; refine the query or explain what is missing.
2. Read the selected service's `crab://owner/name` resource. Use its actual README and input schemas to check required arguments, prerequisites and side effects. If the host cannot read MCP resources, report that limitation instead of guessing a schema.
3. Check the tool's `price`. It is in USD micros: divide by 1,000,000 for USD. An absent price means the tool is free. Respect the user's budget and the host's confirmation requirements.
4. `call` with the returned `service` address, exact `tool` name and schema-conforming `input` object. Call only operations authorized by the user's request.
5. Answer with the result. Preserve relevant refusal details and recovery guidance. When a prerequisite is missing, follow the service's documented setup before retrying.

Do not invent service addresses, tool names, schemas or results. Use existing deployed services for the task; creating or deploying a service is a separate user request.

## If the connection is unavailable

Use the existing plugin's host connection flow and share its active approval link when available. Report any required manual host action precisely. Verify recovery with an authenticated `search`; an empty successful result still verifies access. Never ask for credentials in chat or print them in tool output.
