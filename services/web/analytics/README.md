# Analytics

Analytics reports website traffic, funnels, acquisition, and Stripe revenue. A shared deployment buffers page views until your account next calls it. The service keeps your visits, events, and payments in your account’s caller storage and uses your granted Stripe key to sync revenue.

When the service is running and accessible, call it with Crab or connect your agent through [MCP](https://crabtalk.ai/docs/guides/mcp). After verifying your domain and adding the snippet, this call moves buffered visits into your storage and returns visitor counts and conversion rates for the funnel.

```sh
crab call crabtalk/analytics funnel '{"steps":["/","/pricing*","signup"],"since":0}'
```

If the image is available to your account, select **Deploy** or run `crab deploy crabtalk/analytics` to start a private copy. You can also [deploy from source](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source). A private copy starts with its own storage. Use its running address in calls, app registration, the snippet, and the secret grant.

## Set up tracking

1. Register an anonymous app for the origin that serves the snippet. Include the `www.` prefix in the origin if the site uses it. Keep the app ID for the snippet.

   ```sh
   crab app create site-analytics --origin https://example.com --service crabtalk/analytics --anonymous
   ```

2. Call `claim` for your domain. Publish the returned token as plain text at `https://example.com/.well-known/crabtalk-analytics` and call `claim` again. The bare domain and its `www.` version share one claim. The service checks the bare domain first, then the `www.` host, and returns the URL that served the token. If the bare domain redirects to `www.`, publish the token on the `www.` host. If you previously verified a claim keyed by the `www.` host, call `claim` again to verify the bare domain. If the domain already belongs to another account, the refusal carries a fresh token to publish on either host. A transfer clears visits that were still buffered for the former owner.
3. Add the snippet to your site with the app ID filled in. The site owner manages consent for browser storage.
4. Use an account API key on your backend to call `event` for actions such as signups. Pass the visitor and session IDs from the browser. The event inherits the visitor’s latest channel, or `direct` if the visitor has no earlier hits.

```html
<script>
(() => {
  const app = "<app id>";
  const id = (store, key) => store.getItem(key) || (store.setItem(key, crypto.randomUUID()), store.getItem(key));
  let visitor, session;
  try { visitor = id(localStorage, "crabtalk_vid"); session = id(sessionStorage, "crabtalk_sid"); } catch {}
  const q = new URLSearchParams(location.search);
  const track = (extra = {}) => fetch("https://api.crabtalk.ai/services/crabtalk/analytics/tools/track", {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-App": app },
    body: JSON.stringify({
      domain: location.hostname, visitor, session: session || "none", path: location.pathname,
      referrer: document.referrer || null,
      utm: { source: q.get("utm_source"), medium: q.get("utm_medium"), campaign: q.get("utm_campaign") },
      ...extra,
    }),
    keepalive: true,
  });
  track();
  window.crabtalk = { track: (event, value, currency) => track({ event, value, currency }) };
})();
</script>
```

Single-page apps call `crabtalk.track()` on navigation. If the browser has no stored visitor ID, the service tries the host’s daily client key. CrabTalk Cloud supplies that key when it receives the app call’s client address. If the host supplies no key, visits without a visitor ID are refused.

## Connect Stripe revenue

1. Copy the browser IDs into Stripe metadata as `crabtalk_visitor_id` and `crabtalk_session_id`. For payment-mode Checkout, set both the session’s `metadata` and `payment_intent_data.metadata`. For subscription-mode Checkout, set both the session’s `metadata` and `subscription_data.metadata`.
2. Create a Stripe restricted key with read access to Checkout Sessions, PaymentIntents, Customers, and Invoices. Request a [runtime secret grant](https://crabtalk.ai/docs/guides/secrets#runtime-secret-resources) and approve it on the returned link.

   ```sh
   crab secret resource request crabtalk/analytics "Stripe analytics" --host api.stripe.com
   ```

3. Call `connect_stripe` with the approved `secret_id`. Use `sync_revenue` to import revenue or `channels` to sync and report together.

Stripe sync reads newly created objects in bounded windows and saves progress after each completed window. For an initial import, call `sync_revenue` again while `has_more` is true. `synced_through` gives the oldest completed endpoint cursor in Unix seconds. If Stripe fails, `sync_revenue` includes the error and keeps the completed progress. `channels` still reports from storage and shows `revenue_synced_through` and `revenue_fully_synced` when Stripe is connected.

A window with more than two Stripe pages is split. A single second with over 200 objects at one endpoint cannot be split further and leaves that endpoint's cursor unchanged. An object that completes or is paid after its creation timestamp passes the cursor can be missed. Revenue records gross payments and subscription renewals, excluding later refunds.

## Reports

`funnel`, `channels`, and `stats` take an inclusive `since` and an exclusive `until` in Unix seconds. Omit `until` to use the current time.

- `funnel` counts visitors who completed two to eight page or event steps in order within the window.
- `channels` reports visitors, paying customers, and revenue by currency. Set `by` to `channel` (the default), `source`, `medium`, `campaign`, or `landing`. Each row's `channel` field holds the grouping value. First touch uses the earliest hit. Last touch uses the latest hit before the visitor's first payment in the window, or their latest hit if unpaid. For either model, `landing` uses the first page view in the window among eligible hits. Missing values and unmatched payments appear as `unattributed`.
- `stats` compares totals with the immediately preceding period of equal length and returns populated UTC calendar buckets for the current window. Set `interval` to `day` (the default) or `week` (starting Monday). Visitors and payers are counted distinctly in each period. Conversion is paying customers divided by visitors, including payments without a visit. Revenue and revenue per visitor use minor units by currency. Bounce rate is the share of sessions that start in the period and have exactly one recorded page view, including views outside the period when checking that count. Rates with no denominator are omitted.
- `journey` accepts a Stripe customer ID or a visitor ID in `customer`. It shows each visit's time, landing page, channel, and pages, along with payment records in chronological order. It reports elapsed days from the first visit to the first payment when both exist and the payment follows the visit. The report keeps the earliest 25 visits, 25 pages per visit, and 100 payments, with a flag for each list that was cut. Long lists of visits or payments can reach the response size limit sooner.

Revenue reports use stored Stripe payments. Call `sync_revenue` before `stats` or `journey` to refresh them. `channels` runs a bounded sync when Stripe is connected.

## Storage and limits

The shared buffer keeps up to 10,000 visits per verified domain. Each owner call moves up to 5,000 rows. A retry after a partial commit cannot duplicate hits. Download and delete in **Storage** cover rows that have reached your caller file.

Browser events can be forged. Keep conversion events you need to trust on your backend. Analytics tools are free to call. App calls and storage count toward the applicable account limits.
