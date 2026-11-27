# Where the truth is

Repository paths refer to the public Crab checkout; use the source links when no checkout is available.

| Question | Source |
| --- | --- |
| The CLI, contexts, `crab.toml` | [crates/crab/README.md](https://github.com/crabtalk/crab/blob/main/crates/crab/README.md) |
| The ChatGPT plugin bundle and installation | [plugins/README.md](https://github.com/crabtalk/crab/blob/main/plugins/README.md) |
| A whole working service to start from: its files and the commands that test, build, deploy and call it | https://crabtalk.ai/docs/development/complete-service |
| `#[tool(...)]` and its `query`, `cron`, `webhook`; `#[derive(Schema)]`, `#[event]`, `emit!`, `#[error_code]`, `declare_service!`: what each accepts and generates | https://crabtalk.ai/docs/development/macros |
| Durable `#[tool(events = "name")]` sources, typed event pages and resume cursors | [berm/lang/README.md#durable-event-sources](https://github.com/crabtalk/crab/blob/main/berm/lang/README.md#durable-event-sources), [berm/host/README.md](https://github.com/crabtalk/crab/blob/main/berm/host/README.md) |
| Every `berm_lang` function and type, with its signature | https://crabtalk.ai/docs/development/api |
| Callers, data, HTTP, calls between services, crons and wakes, vars: each one's rules | the capability pages in [Development](https://crabtalk.ai/docs/development/introduction) |
| `crab.toml` runtime keys, `[http]`, secrets, credentials, libraries | https://crabtalk.ai/docs/development/crab-toml |
| `crab bench`, JSON Lines workloads, HTTP fixtures and local cost estimates | https://crabtalk.ai/docs/guides/bench |
| Budget units, measurement, headroom, host ceilings and E0013 | https://crabtalk.ai/docs/guides/budgets |
| `crab dev`, the local host, local secrets, replaying a failed call | https://crabtalk.ai/docs/guides/local |
| Deploying to Cloud, images and registries | https://crabtalk.ai/docs/guides/deploy |
| Secrets on Cloud and asking a person to set them | https://crabtalk.ai/docs/guides/secrets |
| Declaring and using secrets in code | https://crabtalk.ai/docs/development/secrets |
| An error that names a code, as `[E0001]` | `crab explain E0001`, https://crabtalk.ai/docs/errors/introduction |
| Writing a service README for crabtalk.ai | [services/README-guide.md](https://github.com/crabtalk/crab/blob/main/services/README-guide.md) |
| Linking service libraries | https://crabtalk.ai/docs/development/reuse |
| A service or library's API and setup | `services/{connect,agents,web,ops}/<name>/README.md`, or `services/kits/<name>/README.md` for a library |
| Classifying tools for discovery and choosing catalog jobs | https://crabtalk.ai/docs/guides/catalog |
| Building a service from OpenAPI | https://crabtalk.ai/docs/development/openapi; macro reference: [crates/openapi/README.md](https://github.com/crabtalk/crab/blob/main/crates/openapi/README.md) |
