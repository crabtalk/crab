# Website analytics

Count website page views, daily visitors, referring hosts and reported AI crawler requests. Ported from Altlook; each private copy owns its sites, events and execution bill, shared with your account's agents.

## Deploy and add a site

If this listing offers **Deploy your own**, create a private copy without a local build:

```sh
crab deploy --from owner/analytics
crab call you/analytics add_site '{"domain":"example.com","name":"My website"}'
```

Replace `owner` with the template publisher and `you` with your deploying account. No provider credentials are required.

`add_site` registers the website inside your analytics deployment and returns a site object. Use its `id` as `SITE_ID` in the tracker and as `site` in report calls. Each website has its own site ID. To find the IDs of websites already registered:

```sh
crab call you/analytics sites '{}'
```

## Connect your website

In your deploying account's dashboard, open **Apps** and create an [anonymous browser app](https://crab.talk/docs/guides/apps) with:

- **Origins:** your website's origin, such as `https://example.com`.
- **Services:** `you/analytics`, matching your deployment.
- **Anonymous:** enabled. Leave **Issuers** empty; visitors do not need to sign in.

Copy the app ID from the dashboard's **X-App** column and use it as `APP_ID`. You can also find it with `crab app ls`. This public ID lets the tracker call `record` from the allowed origin; it is separate from the `SITE_ID` created by `add_site`.

Copy [tracker.js](https://github.com/crabtalk/crab/blob/dev/services/web/analytics/tracker.js) into your website's public assets so it is served at `/tracker.js`, then add this script to your pages, replacing `you`, `APP_ID` and `SITE_ID`:

```html
<script defer src="/tracker.js" data-endpoint="https://api.crab.talk/services/you/analytics/tools/record" data-app="APP_ID" data-site="SITE_ID"></script>
```

The tracker records page loads and SPA path changes, sends page paths without queries or fragments and only the referring origin, skips common automation and localhost, and stores no browser identifier. Set `data-allow-localhost` to count local development. `localStorage.crab_analytics_ignore = "true"` opts that browser out.

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
