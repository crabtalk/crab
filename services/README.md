# Services

Each directory is a member of the root Cargo workspace. A service builds both a deployable wasm image and a Rust library.

- [Resend](resend/README.md): email sending and delivery status.
- [GitHub](github/README.md): repositories, issues, pull requests and releases.
- [Upstash Redis](upstash/README.md): strings, expiry, counters, pipelines and transactions.
- [PostHog](posthog/README.md): event capture, feature flags and HogQL queries.
- [OSV](osv/README.md): package vulnerabilities and advisory lookup.
- [Release watch](release-watch/README.md): retained release and vulnerability observations.

`crab build` and `crab deploy` operate from a service directory. Agents call deployed tools through MCP.

To reuse source, add the package as a dependency with `default-features = false`. This disables its standalone `tools` exports. Call the Rust functions from your own tools. `berm-build` inherits the dependency's credentials, host permissions and migrations, while its deployment name, tool access, cron settings, variables and database sizing stay with its own deployment. A dependency keeping migrations needs a stable migration ID.

All linked code executes inside the consuming service and uses its secrets and database. Calling a separately deployed service instead uses that deployment's execution and state.
