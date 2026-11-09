# OSV

Find known vulnerabilities matching an installed package version, query multiple packages, or retrieve a full advisory. No provider account or API key is required.

## Call

With Crab signed in to Cloud, replace `owner/osv` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/osv query '{"package":{"ecosystem":"crates.io","name":"serde","version":"1.0.0"}}'
```

Returns OSV advisory records and an optional `next_page_token`. Use `query_batch` for multiple packages and `advisory` for full details by advisory ID.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls run on the selected deployment; no provider credential is needed.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/osv
```

There are no provider credentials or required settings. A private copy keeps its execution and bill under your account.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

Use the ecosystem’s exact name, package name and installed version. The service does not resolve lockfiles, commits or package URLs.

Pass `next_page_token` as `page_token` for the next request, including when a page contains no records. Batch results have a separate token per input.

No matches means OSV returned no known matching records; it is not proof that a package is secure. Calls are not automatically retried.

[Rust library integration](https://github.com/crabtalk/crab/blob/main/services/connect/osv/LIBRARY.md) · [Provider documentation](https://google.github.io/osv.dev/)
