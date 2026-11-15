# Service READMEs

A deployable service's README appears on crabtalk.ai and in MCP service resources. Write for someone choosing a tool, calling a deployment, or deploying a private copy. The service page already supplies tool descriptions, argument schemas, prices and deployment controls.

## What to include

- Start with the outcome the service provides. Say whose provider account and state a call uses.
- Show one useful `crab call owner/name tool '<JSON>'` example. Explain the address placeholder, required setup, side effects and what comes back. Check the tool name and input against the exported schema, including any `input`, `query` or `body` nesting. Do not invent output or execute paid or mutating calls to check an example.
- Explain the choice between calling an accessible running deployment and deploying a private copy. A template listing is not necessarily callable. Mention MCP with a link to the shared connection guide.
- For a published template, point to **Deploy** and show `crab deploy --from owner/name`. Make availability conditional unless publication is verified. Describe required credentials, variables, provider permissions and state ownership. Link to source deployment as a fallback; do not require a local build to consume a published image.
- Keep behavior that changes how a person uses results: pagination, asynchronous job follow-up, partial failures, quotas, retry/idempotency rules and significant omissions. Use the service's actual boundaries, not a provider's whole feature list.
- Use absolute links that work on crabtalk.ai and GitHub. Link repository files on `main`; pin Rust dependencies to a released tag matching `berm-lang`. Never link or depend on `dev`. Keep paragraphs on one line. Keep setup steps and examples short; do not duplicate every tool schema or price table.

## What belongs elsewhere

Rust dependency examples and library APIs belong in `LIBRARY.md` or the source docs, linked at the end. Generator details, upstream revision pins, catalog mappings and coverage comparisons belong in source/build files or contributor docs. Historical scope labels such as “treg coverage” do not explain a service to its users.

Library-only crates have no standalone calls or deployment. Label them as libraries and document integration into a consuming service; do not give them a fictitious Cloud workflow.

## Before finishing

Read the service's `crab.toml`, exported tools, input types and relevant tests. Check examples against the built image's schema when available. Verify credentials, variables, cron behavior and state ownership. Recheck this README when any of those change. Validate Markdown and links without making live provider calls.
