# PostHog

Capture events, evaluate feature flags and run HogQL analytics queries in the deployment’s PostHog Cloud project. US and EU Cloud are supported.

## Call

With Crab signed in to Cloud, replace `owner/posthog` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/posthog query '{"sql":"select event, count() from events group by event limit 10","name":"Event totals"}'
```

Returns the provider’s query result, including columns, rows and pagination metadata. Use `capture` or `capture_batch` for events and `flags` for flag evaluation.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/posthog
```

Set `POSTHOG_REGION` to `us` (default) or `eu`. Capture and flags need `POSTHOG_PROJECT_TOKEN`. Queries need the numeric `POSTHOG_PROJECT_ID` and `POSTHOG_PERSONAL_API_KEY` with `query:read` and access to that project. Credential groups are optional; set the ones your tools use. Keep the copy private when the project is for your own use.

Request the optional credential groups for the tools you use:

```sh
crab secret request posthog --credential posthog_private
crab secret request posthog --credential posthog_project
```

Set the copy’s variables as needed:

```sh
crab var set posthog POSTHOG_REGION eu
crab var set posthog POSTHOG_PROJECT_ID 123
```

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source).

## Usage notes

Event acceptance does not mean the event is immediately queryable. Use a stable event UUID when resubmitting. `distinct_id` must have 1–200 characters.

Inspect `errors_while_computing_flags`; missing flags are unknown and partial results are not a complete evaluation. Flag evaluation does not capture `$feature_flag_called`.

Queries run blocking HogQL. Include a descriptive name and a SQL `LIMIT`. Pagination, polling and retries are explicit. Self-hosted PostHog is not supported.

[Rust library integration](https://github.com/crabtalk/crab/blob/dev/services/posthog/LIBRARY.md) · [Provider documentation](https://posthog.com/docs/api)
