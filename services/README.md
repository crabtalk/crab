# Services

Each crate is a member of the root Cargo workspace. Deployable services build both a wasm image and a Rust library. Reusable libraries are available through [crabkit](../crates/crabkit/README.md).

- [Resend](resend/README.md): email sending and delivery status.
- [GitHub](github/README.md): repositories, issues, pull requests and releases.
- [Upstash Redis](upstash/README.md): strings, expiry, counters, pipelines and transactions.
- [PostHog](posthog/README.md): event capture, feature flags and HogQL queries.
- [CompanyEnrich](companyenrich/README.md): generated company intelligence, people discovery and enrichment jobs.
- [People Data Labs](pdl/README.md): generated person and company enrichment and search.
- [DataForSEO](dataforseo/README.md): generated keyword metrics, backlinks, rankings and web data tools.
- [Context.dev](contextdev/README.md): generated brand intelligence, extraction and web tools.
- [Firecrawl](firecrawl/README.md): bounded scraping, search, mapping and crawl jobs.
- [Litescrape](litescrape/README.md): generated search, maps, reviews and web tools.
- [AnyAPI](anyapi/README.md): generated social data, search and web tools.
- [TinyFish](tinyfish/README.md): OpenAPI-backed web, news and publication search, multi-URL extraction and asynchronous browser agents.
- [OSV](osv/README.md): package vulnerabilities and advisory lookup.
- [Release watch](release-watch/README.md): retained release and vulnerability observations.

`crab build` and `crab deploy` operate from a service directory. Agents call deployed tools through MCP.

To reuse a deployable service as a library, add the package as a dependency with `default-features = false`. This disables its standalone `tools` exports. Call the Rust functions from your own tools. `berm-build` inherits the dependency's credentials, host permissions and migrations, while its deployment name, tool access, cron settings, variables and database sizing stay with its own deployment. A dependency keeping migrations needs a stable migration ID.

All linked code executes inside the consuming service and uses its secrets and database. Calling a separately deployed service instead uses that deployment's execution and state.

## Building from source

Calling an existing deployment or deploying a published template needs no local build. To modify a service or deploy one without a published template, install [Crab](https://crab.talk/docs/guides/quickstart#1-install-the-cli) and a Rust toolchain, clone this repository, and select the service directory:

```sh
git clone https://github.com/crabtalk/crab
cd crab/services/github
crab login
crab deploy
```

Replace `github` with the service to build. `crab deploy` builds the image, adds the WebAssembly target through rustup if needed, and asks for approval on Cloud. Set the credentials and variables documented in the service's README. Generated integrations may need `curl` and network access to fetch their pinned provider specifications during the build.

For local development, use `crab --context default deploy`, set local credentials with `crab --context default secret fill github`, and pass `--caller alice` when calling a public tool. See [local development](https://crab.talk/docs/guides/local).

## Documentation

Follow the [service README guide](README-guide.md) for READMEs displayed on crab.talk. Rust integration examples for deployable services live in their `LIBRARY.md` files when provided.
