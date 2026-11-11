# GitHub

Read repositories, issues, pull requests and releases, or create issues, comments, pull requests and releases using the deployment’s GitHub access.

## Call

With Crab signed in to Cloud, replace `owner/github` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/github latest_release '{"owner":"rust-lang","repo":"rust"}'
```

Returns the latest published release, excluding drafts and prereleases. Use `releases` for release history, or `issues` and `pull_requests` for repository work.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/github
```

Public repository reads need no credential. For private repositories or writes, set `GITHUB_TOKEN` through the copy’s credential setup. Use a fine-grained token scoped to the repositories you need: Metadata for repository reads, Issues for issues, Pull requests for PRs, and Contents for releases. Grant write access only for operations you intend to use.

For private repository access or a higher GitHub rate limit, request the optional GitHub credential explicitly:

```sh
crab secret request github --credential github_api
```

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

Lists return `items` and `next_page`; pass the next page with the same filters and size. The default page size is 30, capped at 100. GitHub’s issue lists can contain pull requests, identified by `pull_request`.

Each operation makes one request. Errors include GitHub’s status, message and supplied retry/reset timing. Writes are not automatically retried.

[Rust library integration](https://github.com/crabtalk/crab/blob/main/services/connect/github/LIBRARY.md) · [Provider documentation](https://docs.github.com/en/rest)
