# Documentation

- Update affected documentation in the same change as code, API, CLI, configuration, repository layout, or platform support changes. A change is unfinished while its documentation describes the previous behavior.
- Search the whole repository for affected concepts, commands, examples, paths, and claims before editing. Review `skills/crab/SKILL.md`, the root and crate READMEs, and relevant guides and examples; update each affected source together.
- CrabTalk website guides live in `../crabtalk/www/site/content/docs/`. Check and update affected guides alongside repository docs.
- Verify documented behavior against the implementation. Update or remove stale instructions and claims, and check links and example paths after moves. Report documentation that needs updating outside this repository.
- Public documentation describes reusable services and Rust libraries. Do not present “kits” as a separate product or deployment model; retain literal crate and ABI names where the implementation requires them.

# Catalog classification

- When adding or changing a deployable service, check that every tool with `access` has an appropriate `catalog` job. Internal cron and webhook handlers remain unclassified.
- Verify platform prefixes and category assignments in `crates/sdk/catalog.toml`. Update the category and example-service tables in `../crabtalk/www/site/content/docs/guides/catalog.mdx` alongside registry or job changes.
- Run `cargo nextest run -p crabtalk-sdk --test catalog` after changing catalog metadata.

# Service OpenAPI sources

- Fetch provider OpenAPI documents at build time into `OUT_DIR`. Never check downloaded provider specs into the source tree; keep source URLs and reviewed revision or digest pins in code.
- Before opening or updating a PR, refetch services' upstream OpenAPI sources into a fresh temporary directory. Do not rely on cached downloads in `OUT_DIR`.
- Compare fresh downloads with the pinned revisions or digests. Review contract changes before updating pins, bindings, tests, or documentation; never refresh a digest without reviewing the diff.
- Run affected service API tests with `cargo nextest run -p <package>` and build its image with `crab build <service-directory>` using the reviewed inputs.

# Test scope

- Cargo `workspace.default-members` defines the default CI test and Clippy scope. Keep generator coverage in `crab-openapi` and handwritten service behavior in the owning service tests.
- Validate generated provider adapters explicitly when changing or publishing them; do not add per-provider Wasm export or schema snapshots to the default CI suite.
