# Website analytics

Count website page views, daily visitors, referring hosts and reported AI crawler requests. Ported from Altlook; each private copy owns its sites, events and execution bill, shared with your account's agents.

## Deploy and add a site

If this listing offers **Deploy your own**, create a private copy without a local build:

```sh
crab deploy --from owner/analytics
crab call you/analytics add_site '{"domain":"example.com","name":"My website"}'
```

Replace `owner` with the template publisher and `you` with your deploying account. Save the returned site `id`. No provider credentials are required. Register an [anonymous browser app](https://crab.talk/docs/guides/apps) restricted to your website origin and this deployment; its public app id lets a browser call `record` without an API key. Use `tracker.js` from this service's source as the page script:

```html
<script defer src="/tracker.js" data-endpoint="https://api.crab.talk/services/you/analytics/tools/record" data-app="YOUR_APP_ID" data-site="SITE_ID"></script>
```

Host that file with your website. It records page loads and SPA path changes, sends page paths without queries or fragments and only the referring origin, skips common automation and localhost, and stores no browser identifier. Set `data-allow-localhost` to count local development. `localStorage.crab_analytics_ignore = "true"` opts that browser out.

## Read reports

```sh
crab call you/analytics days '{"site":"SITE_ID"}'
crab call you/analytics pages '{"site":"SITE_ID","limit":20}'
```

`days` returns daily views, daily visitors, AI-referred views and crawler requests. `pages` and `sources` rank page paths and referring hosts; visitors summed across days are visitor-days, not unique people across the range. Visitor counts require the host's daily client key, so server calls without it record views without visitors. The same copy is callable from your agents through [MCP](https://crab.talk/docs/guides/mcp).

`events` returns raw events with a cursor: pass `next` as `after`. `crawlers` groups crawler reports. To report crawler requests from your website's server, call `record` with the same `site`, a `page` path and the request's `agent` user-agent string. Only recognized AI crawler labels are kept. Labels come from user-agent text, not verified bot identity. Browser collection alone cannot see crawler requests.

## Limits

Twenty sites per copy. Hourly cleanup removes raw events older than thirty days and daily rollups older than 366 days, in batches of 1000 expired rows per table. Reports default to thirty days and accept an ordered range of up to 366 days. Ranked and raw-event lists return at most 100 rows.

Paths are at most 1024 bytes; referrers at most 4096 bytes; user-agent strings at most 1024 bytes. A copy holds at most 50000 raw events and 50000 daily page/source/crawler combinations within its 128 MiB database. At capacity, new records fail explicitly until cleanup or site removal frees space. Recording has no automatic retry or event deduplication.

Keep the deployment private. The anonymous tool records events; the authenticated report and management tools share the account's data. Use an origin-restricted app and platform request limits to control collection. This template does not include Altlook's dashboard or migrate an existing Altlook database.

[Source deployment](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/dev/services/web/analytics/LIBRARY.md)
