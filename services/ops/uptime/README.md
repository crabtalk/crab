# Uptime

Check your HTTP endpoints on a schedule and retain their observations. Each private copy owns its monitors, history and execution bill.

## Deploy and watch a URL

If this listing offers **Deploy**, deploy a copy without a local build:

```sh
crab deploy --from owner/uptime
crab call you/uptime watch '{"name":"API","url":"https://example.com/health","interval_minutes":5}'
crab call you/uptime check '{"name":"API"}'
```

Replace `owner` with the template publisher and `you` with your deploying account. No provider credentials are required. Any public HTTP(S) endpoint can be monitored; it needs no particular health-response JSON format. Default success is HTTP 2xx. Set `expected_status` to require one exact status, such as 204. Redirects are observed without following them.

`monitors` returns private target details and the latest result. `history` pages through observations with `after` and `next`. Two consecutive failed checks mark a monitor down; one successful check restores it. A target not checked yet is unknown.

## Status-page data

`status` returns only monitor names, states, check times and latency, omitting target URLs and errors. It marks observations stale when older than twice their configured interval. Stale or missing results must not be displayed as healthy.

A status page can use a registered [anonymous browser app](https://crabtalk.ai/docs/guides/apps) to query this summary while the deployment stays private. Anonymous app calls cannot reach the authenticated management tools. The template supplies status data, not a hosted status-page website or alert delivery. Agents use its tools through [MCP](https://crabtalk.ai/docs/guides/mcp).

## Limits

Twenty monitors per copy, at intervals of 5–1440 minutes. A minute cron checks at most five due monitors, ordered by due time; scheduling is best effort. Each GET has a five-second timeout and reads at most 1 KiB of body. Latency measures the bounded GET, not full page download or browser rendering.

History retains the newest 1000 observations per monitor and removes older-than-thirty-day observations on its next check. Repeating `watch` for the same target preserves history; changing its URL or expected status resets it. `unwatch` removes the monitor and its history. The database reserves 64 MiB.

Checks run from this service's host, without multiple-region quorum, content assertions or authenticated probes. Cloud blocks private and loopback targets. Keep the deployment private; publishing it would also expose its management tools.

[Source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/main/services/ops/uptime/LIBRARY.md)
