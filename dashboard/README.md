# berm dashboard

A development dashboard for bermd, over its control API: deployed programs, calling their tools, calls, logs, crons and wakes, usage and meters.

```sh
bun install
bun run dev                                  # proxies /programs to http://127.0.0.1:7777
BERMD=http://127.0.0.1:7788 bun run dev      # or another bermd
bun run build                                # static files in build/
```

bermd carries no authorization and verifies no `X-Berm-Caller`; neither does the dashboard.
