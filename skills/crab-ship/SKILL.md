---
name: crab-ship
description: Deploy, publish and administer CrabTalk services on Cloud with the Crab CLI. Use for Cloud sign-in, deployment approval, plans and credit, provisioning secrets, granting priced-tool access, payouts, images or deployment settings.
---

# Ship and operate on Cloud

## The person approves by link

Share the active link with the person and explain what it deploys, buys or grants, including any price. Keep device login and Cloud deploy commands running while the person decides because those commands follow their own requests.

For a Billing, Secrets or key access request that returns a code, run `crab request <code> --wait`. It succeeds on completion and times out after 600 seconds by default. Check the same request if it is still pending. Use `crab request <code> --expire` to end an incorrect request before replacing it. Secrets already saved stay set. Ask before replacing an expired or declined request.

For a tool's service request, follow the recorded outcome via its authenticated MCP resource or `crab request <code> --wait`. A builder prepares the review. The person's browser approval starts execution. After a failure, have the person reopen the same review within its execution window. Check the outcome before creating a replacement. See [Service requests](https://crabtalk.ai/docs/guides/service-requests). For a host-managed MCP connection approval, use the installed client's flow and keep it open until completion or expiry. See [MCP connectors](https://crabtalk.ai/docs/guides/mcp#connectors).

For CLI refusals that name an explanation code, use `crab-debug`.

## Signing in

Sign in with `crab login --device`. The command prints a link and code, then waits up to 30 minutes while the person reviews the connection name, account, permissions and daily budget and approves a dedicated key. They can also enter the code at `crabtalk.ai/device` and later manage the key in Settings → API keys. Plain `crab login` requests a personal session and opens the approval page in a browser.

## Plans and credit

- Personal Free accounts can deploy after GitHub verification while their compute grant lasts. Run `crab billing verify`. If it returns a link, the person connects GitHub to their account there. The default offer is a one-time $1 compute grant valid for 30 days, with storage included. The Cloud can configure the offer and retention. Monthly resets and plan changes never replenish the grant. Go enables hosting if verification fails. Organization Free accounts need a paid plan or a comped entitlement to host. See [Plans](https://crabtalk.ai/docs/guides/plans).
- `crab plans` lists Cloud plans with the compute each includes a period. `crab rates` shows the rates, the same on every plan. `crab billing` shows the plan's compute used of what it includes, any overage, prepaid and what of it expires soon, and when a held account's services are retired. Included usage pays compute only; Go's compute past it is charged to the card in $5 steps. Priced tools and rotating IPs are paid from prepaid, which Free cannot use.
- To buy a plan or credit, run `crab billing plan <slug>` or `crab billing credit` with `--for` saying what it is for. The code lasts 30 minutes.
- When execution funding ends, services pause. The default recovery window is 30 days. Upgrade before it ends, or use `crab export OWNER/NAME` or Download in the service's Settings. The SQLite export contains SQL and key-value data and excludes workflow runs and checkpoints.

## Deploying

- `crab up [dir]` builds and deploys your own source. Repeat `-p <name>` to select workspace services. `crab deploy owner/name --version 1.2.3` deploys a published image. On Cloud, the approval link lasts 15 minutes and a newer deploy of the same service replaces it.
- Personal CLI sessions always need browser approval. For direct deployment, the owner can grant the device key `deployments:write`. `--yes` requires an API key with `deployments:write`; use it only when the person asked you to deploy without their review. When asking through the API, the deploy request's `context` is one short sentence describing the change, at most 160 characters.
- For pending approvals and recorded outcomes, use `crab-debug`.
- When redeploying a published image on Cloud, the host keeps a compatible existing storage reservation. A source redeploy sends the reservation from its build. See `crab-build` for storage sizing and tool budgets.
- `crab stop <service>` and `crab start <service>` stop and serve a service, keeping its data; `crab rm <service>` removes it and its data. A Cloud CLI session opens an owner approval request. An API key with `services:write` removes it directly. Stop only when the person asks.

## Secrets

Keep secret values out of chat and tool output. Read provisioning status by name.

- On Cloud, once the service is deployed, ask for its unset required secrets in one request: `crab secret request <service>` with `--for` saying what they are for and `--copy KEY=VALUE` for each value the credential's guide has the person enter at the provider, such as `--copy callback=<URL>`. `--credential <id>` asks again for one, as after rotating it. Follow the approval protocol above.
- For local development, ask the person to run `crab --context default secret fill <service>` in their own terminal. It needs a terminal for both input and output. Pass `--copy KEY=VALUE` for provider setup values.
- For secrets created after deployment, use account-owned secret resources: the service declares `secret_resources = true` and its HTTP destinations. Run `crab secret resource request <service> "Research Bot" --host api.example.com`, send the link, and wait. The command returns a secret ID, never a value; `--id <id>` requests rotation of an existing resource. Store the ID and use `{secret:ID}` in requests. Grants never expand HTTP permissions. For local resources, the person uses `crab --context default secret resource` in their own terminal with `create <name>`, `grant <id> <service> --host <host>`, `rotate <id>`, `revoke <id> <service>` or `rm <id>`.
- `crab secret ls <service>` lists names and hosts. API keys cannot set fixed values or destinations. Changing a fixed destination takes a manifest change, redeployment and fresh provisioning. Full guide: [Secrets](https://crabtalk.ai/docs/guides/secrets).

## Grants, prices and payouts

- An owner grants a key access to another account's priced service. The key's budget for tool and workflow prices defaults to $5 per UTC day. Use any request link returned by a refusal, or ask in advance with `crab key ask --service <owner>/<name>` or `crab key ask --budget <usd>`. Use the subcommand's `--context` to explain the request. After approval, retry the refused call if the user still wants it.

- A deploy that sets prices is refused until the account has a payout account that can receive payouts, a US one. The account owner can start Stripe onboarding with `crab earnings connect`, which prints the URL and tries to open it in their browser. Deploy after Stripe enables payouts. `crab earnings` shows each month's revenue, cut, compute and net.
- `crab sunset <service> <tool> <YYYY-MM-DD>` stops serving a deprecated tool from that UTC day; without a day it keeps serving it.

## Images

- `[image] name = "owner/service"` names an image in Cloud. `deploy = true` lets other accounts download and deploy it; `public` is the default visibility of its deployments. Both default to false, independently of tool access.
- `crab push [dir]` builds and publishes images and accepts repeatable `-p <name>`. `crab pull owner/name:1.2.3` stores one locally. Versions are immutable; three images remain available for download, and existing deployments keep their copies. Deployable images allow anonymous downloads. Download private images using the publisher's Cloud context. Local `crab deploy` uses the stored image and pulls it if needed.
- `crab deploy owner/name` of another account's image takes `--as` for the instance name, `--public`/`--private` and repeatable `--price tool=micros`. Omitted settings keep the existing overrides.
- `--auto-upgrade` opts a Cloud image deployment into compatible stable patches within the same major and minor version. Permission and migration changes can require manual review.
- See [Deploying](https://crabtalk.ai/docs/guides/deploy) and [Publishing](https://crabtalk.ai/docs/guides/publishing).

## Vars

`crab var set <service> <var> <value>` sets a value read from the next call. `crab var unset <service> <var>` restores the default.
