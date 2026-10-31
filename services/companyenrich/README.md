# CompanyEnrich

41 tools covering the treg CompanyEnrich scope: company enrichment, workforce, search and lookalikes; people discovery and email resolution; batch enrichment, async exports, job status, taxonomies, geography and account information.

Set `COMPANYENRICH_API_KEY` from the [dashboard](https://companyenrich.com/dashboard). Authentication uses a host-managed Bearer token. Search, enrichment and async jobs consume credits; email enrichment endpoints are beta. Calls submit one request: no automatic pagination, polling, retries or result downloads.

`build.rs` downloads the official [endpoint OpenAPI definitions](https://docs.companyenrich.com/llms.txt), verifies the page digests in `sources.sha256`, and merges their paths and components in `OUT_DIR`. Source changes require reviewing and updating the corresponding digest. No downloaded documents are committed.

The 29 company/person tools have catalog mappings. Job, taxonomy, geography and account tools are directly callable without catalog mappings. Saved-list CRUD, the standalone education endpoint and similar-company preview are outside the selected scope. Education and workforce expansion options on selected endpoints are retained.

Use `default-features = false` for Rust-only `api` and `types` modules. Responses retain upstream JSON and shared HTTP errors; success response headers are not returned. Generated schemas retain upstream constraints; Rust deserialization checks types, not every constraint.
