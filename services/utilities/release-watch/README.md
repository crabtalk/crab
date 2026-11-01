# Release watch

Track release and vulnerability observations for installed package versions. Keep a watchlist, check it on a schedule, and read retained changes through CrabTalk.

## Call

With Crab signed in to Cloud, replace `owner/release-watch` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/release-watch follow '{"watch":{"name":"serde","package":{"ecosystem":"crates.io","name":"serde","version":"1.0.0"},"owner":"serde-rs","repo":"serde"}}'
```

Adds or updates a watch; it does not fetch immediately. Call `refresh` with `{"name":"serde"}` to check it now, then `changes` with `{"after":0}` to read observations. `watches` shows status; `unfollow` stops checks while keeping history.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials and shared state.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/release-watch
```

Keep the copy private: its watchlist and history are shared by every authorized caller, not partitioned by caller. An optional `GITHUB_TOKEN` improves GitHub rate limits and permits authorized private-repository reads. OSV needs no key. GitHub and OSV are linked libraries; you do not deploy them separately.

For private repository access or a higher GitHub rate limit, request the optional GitHub credential explicitly:

```sh
crab secret request release-watch --credential github_api
```

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source).

## Usage notes

Supply exact installed versions and explicit GitHub repository mappings. A deployment holds up to 20 watches. Its cron runs every 15 minutes and checks up to five watches due for an hourly check; manual refreshes consume provider quota immediately.

The first successful check records a `baseline`. Later observations are `changed`, `failed`, or `recovered`. Failures retain the last successful snapshot; repeated identical failures add no history. Changing a mapping or installed version starts a new baseline.

Read `changes` using `next_cursor` and `has_more`; reads do not acknowledge or delete history. GitHub evidence covers its first recent release page, up to 30 entries; `releases_next_page` signals older pages. OSV pagination failures preserve prior data.

The service does not compare package versions with repository tags, infer mappings, recommend upgrades or establish that a release fixes an advisory. History has no automatic cleanup.

[Rust library integration](https://github.com/crabtalk/crab/blob/dev/services/utilities/release-watch/LIBRARY.md) · [Provider documentation](https://google.github.io/osv.dev/post-v1-query/)
