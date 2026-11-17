# Page Watch

Check up to three public HTML pages, report which Markdown snapshots changed, and save new snapshots only on the changed branch. State belongs to the deployment and is shared by its callers. No provider account or credentials are required; the service links the existing Web Markdown library. It reads server HTML without executing JavaScript.

Call an accessible deployment with a workflow-capable Crab CLI/runtime:

```sh
crab workflow inspect OWNER/page-watch.page_watch
crab workflow run OWNER/page-watch.page_watch '{"urls":["https://example.com/"]}'
crab workflow status OWNER/page-watch.page_watch RUN_ID
```

Replace `OWNER/page-watch` with the deployment address. A first observation counts as a change. A successful result has `checked`, `changed` (URLs), and `saved`. Each extracted page is limited to 128 KiB by the Markdown library; more than three input URLs fails before HTTP dispatch. An unchanged run skips the save tool.

The configured customer task price is 1,000 USD micros ($0.001), settled only on success. Cloud owner calls and schedules have no retail sale; the local runtime does not settle task fees. The separate Cloud managed-spend cap is 100,000 USD micros ($0.10) across all attempts. Read the current quote before agreeing to a run. Running deployments can also be used through [MCP](https://crabtalk.ai/docs/guides/mcp).

## Run your own copy

This example is not configured as a published image. If a publisher makes a template available, use its Deploy control or `crab deploy OWNER/TEMPLATE`; consuming an existing deployment or template needs no local build. To run this source example from the public repository checkout:

```sh
cd services/ops/page-watch
crab dev -d
crab --context default deploy --build
crab --context default workflow inspect page-watch.page_watch
crab --context default workflow run page-watch.page_watch '{"urls":["https://example.com/"]}'
```

Use a new run to observe current page content. `workflow status page-watch.page_watch RUN_ID` shows the saved results, attempts and seven-day expiry. After a known fetch failure is corrected, `workflow continue page-watch.page_watch RUN_ID` reuses successful reads and the original input. An uncertain write to `save` has no declared idempotency window and refuses continuation. Start a new run when eligibility says continuation is unavailable. Cancellation prevents new calls and cannot undo a saved snapshot.

To enable a weekly Monday 08:00 UTC run, uncomment the schedule block in `crab.toml`, choose the URLs, and redeploy. Scheduled runs use owner funding, have stable occurrence identities, and do not retry failures or backfill missed occurrences.

## Authoring and checks

`#[tool]` generates the local `ReadCall`, `CompareCall` and `SaveCall` descriptors used by the workflow. Their argument fields and output types come from the real tool signatures; no hand-written client schema or separate Markdown deployment is needed. `webpage.workspace = true` links the existing library with its standalone tool exports disabled by the workspace dependency. See [Workflow authoring](https://crabtalk.ai/docs/development/workflows).

The read and comparison tools are queries. Continuation may retry a failed read; successful reads remain checkpointed. The save tool mutates shared state and declares no retry guarantee.

The real-host tests use arranged HTML responses and never call public providers. Build the image before running them from this repository root:

```sh
crab build services/ops/page-watch
BERM_TEST_IMAGE_CRAB_PAGE_WATCH="$PWD/target/berm/page_watch.wasm" cargo nextest run -p crab-page-watch --test workflow --run-ignored all
```

These tests are explicitly selected because the shared fixture setup does not prepare this example image. The CLI build output gives the actual image path when `CARGO_TARGET_DIR` is set.
