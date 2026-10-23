---
title: Running the Service
description: "crab dev deploys programs and serves their tools over HTTP, and their anonymous tools over MCP."
---

`crab dev` runs the local host, a long-running service that deploys programs and serves their tools.
It is the one host on a machine: `-d` starts it in the background, writing to `host.log` under its root, as does `crab deploy` on the `default` context when nothing answers on `127.0.0.1:7777`. `crab dev status` and `crab dev stop` reach it at `/dev`.
An invocation is ephemeral, but three things around it are not: the deployed
set, the modules compiled from it, and the engine's code cache. A program is
compiled once, at deploy, so an invocation pays only instantiation.

```sh
crab dev --addr 127.0.0.1:7777 --root ~/.crab/host
```

| Flag | |
|---|---|
| `-d` | Run it in the background and return once it answers. |
| `--addr` | Where it listens, `127.0.0.1:7777` by default. |
| `--root` | Where deployed images, the code cache and `secrets.toml` live, `~/.crab/host` by default. |
| `--max-call-depth` | How deep a chain of programs calling programs nests, 4 by default; `0` refuses the first nested call. |
| `--max-cpu-millis` | How many milliseconds of CPU one call spends, nested calls excluded, 100 by default. |
| `--max-call-seconds` | How long one call runs before it is stopped, 60 by default. |
| `--proxy` | The proxy `berm.http.proxy` goes through, e.g. `http://user:pass@gate.example.com:3120`. Without one it goes direct. |

The service carries no authorization and binds loopback by default.

## Docker

`docker-compose.yml` at the repository root builds the image from the checkout and runs `crab dev` on `127.0.0.1:7777`, its root the volume `berm` at `/data`:

```sh
docker compose up --build
```

Inside the container the host listens on every interface; a port published without `127.0.0.1:` is open to the network. `command:` in the compose file passes `crab dev` its other flags: `--proxy`, `--max-call-depth`, `--max-cpu-millis`, `--max-call-seconds`.

## Deploying

```sh
curl -X PUT --data-binary @program.wasm 'http://127.0.0.1:7777/programs/example?public=echo'
curl http://127.0.0.1:7777/programs
curl -X DELETE http://127.0.0.1:7777/programs/example
```

- `PUT /programs/{name}` deploys the image in the body under `name`, replacing
  what is there, and answers `{"program", "breaks"}`: the IDL, and how it broke
  callers of the program it replaced.
- A name is lowercase letters, digits, `-` and `_`.
- `?public=a,b` lists the tools the deploy may make public; a deploy that makes
  one public that it does not list is refused with `409`.
- `?crons={"tick": "*/5 * * * *"}` sets each cron tool's expression. A deploy
  that would undo one set since the last deploy that set them is refused, and
  so is one that breaks a tool or event a caller used in the last 30 days
  ([the manifest](./manifest.md#compatibility)), unless `?force=true`.
- A deploy is recorded only once its image has compiled. What is recorded is
  restored on the next start, and an image that no longer loads is reported and
  skipped.

## Reading a program

| Route | Answers |
|---|---|
| `GET /programs` | Every deployed program's IDL. |
| `GET /programs/{name}` | Its IDL under `program`, and under `served` the calls of its tools from outside that ended done, per day over the last 30 days and in total. |
| `GET /programs/{name}/idl` | The IDL alone: name, version, digest, package fields and public tools. |
| `GET /programs/{name}/runtime` | Its other tools, `deps`, `events`, and the `deps` nothing here answers to. |
| `GET /programs/{name}/terms` | Each public tool's budget, as `{"tools": {"<tool>": {"budget": {"cpu", "rows"}}}}`. |
| `GET /programs/{name}/usage` | Each caller of each tool and event over the last 30 days. |
| `GET /programs/{name}/live` | What it is using now: invocations running, memory, bytes stored, and each resource's total and rate. |
| `GET /programs/{name}/meters` | Its meters over the last 30 days, a day each, with the bytes it stored. |
| `GET /programs/{name}/calls` | Each run, a call from outside or a wake, newest first, with its outcome, duration and meter. `?tool=&event=&outcome=&since=&until=&before=&limit=`: the calls below the id `before`, and `next` in `Link`. |
| `GET /programs/{name}/calls/{id}/input` | What a call that failed ran with: its tool, arguments as sent, caller, and the image's `digest`. `404` for a call that kept none; no other call does. |
| `GET /programs/{name}/calls/{id}/tape` | A call that failed, as its answers from outside the program were recorded, encoded as `berm::tape::Tape`. `404` for a call that kept none. |
| `GET /programs/{name}/logs` | What it emitted, oldest first, each beside the call it was emitted in and kept 30 days. `?topic=&call=&after=&limit=`: the logs after the sequence number `after`, and `next` in `Link`. |
| `GET /programs/{name}/crons` | Each cron tool with its expression and next run. |
| `GET /programs/{name}/wakes` | The wakes armed to call its tools that have not run. |

## Calling a tool

```sh
curl -X POST -H 'X-Berm-Caller: {"id": "alice"}' -d '{"query":"hi"}' \
  http://127.0.0.1:7777/programs/example/tools/echo
```

- `POST /programs/{name}/tools/{tool}` takes the argument object as its body and
  answers `{"done": "<result>"}` or `{"failed": "<message>"}`.
- A tool marked `query` also answers
  `GET /programs/{name}/tools/{tool}?args=<json>`, the argument object
  URL-encoded, with the program's commit count as its `ETag`; `If-None-Match`
  with it answers `304`. Any other tool answers `GET` with `405`.
- `X-Berm-Caller` is the caller, as JSON:
  `{"id", "app", "user": {"issuer", "subject"}, "client", "agent"}`, every field
  but `id` optional. The local host checks none of it. A call without one, or an app's
  call without a `user`, has no caller and reaches only `anonymous` tools.
- The answer carries `Berm-Meter`, what the call used across every program it
  reached: `{"calls", "cpu", "memory", "read", "written", "proxied", "logged", "failed", "sent"}`.
  A deprecated tool's answer carries `Deprecation`, and `Sunset` once one is set.

| Status | When |
|---|---|
| `401` | No caller, and the tool is not `anonymous`. |
| `403` | The tool is `callers(services)`. |
| `404` | No such program, or no public tool by that name. |
| `405` | `GET` on a tool that is not a query. |
| `503` | No call slot is free. |

## Budgets

An invocation stops at its tool's CPU budget, checked every 100ms, and a row syscall it makes once it has read and written its row budget together is refused: `--max-cpu-millis` of CPU, 100 by default, and 1,000,000 rows, unless `PUT /programs/{name}/tools/{tool}/budget` set `{"cpu", "rows"}` lower, `cpu` in microseconds, from the next call. `cpu` in `Berm-Meter` is microseconds too, each invocation's own, nested calls excluded.

## Crons, deprecation and Stripe events

- `PUT /programs/{name}/tools/{tool}/cron` sets a cron tool's expression as
  `{"cron": "*/5 * * * *"}`, five fields in UTC; `null` stops it.
- `PATCH /programs/{name}/tools/{tool}` with `{"sunset": <epoch seconds>}` sets
  when a deprecated tool stops being served; `null` clears it.
- `POST /programs/{name}/stripe` hands its body, a Stripe event, to the
  program's `stripe` tool, unverified, and answers `204` once the tool has
  run, `503` while the program is held and `500` when the tool failed.

## Secrets

A request header or body may name `{secret:NAME}` for a secret the manifest declares.
The local host substitutes the value it keeps for that program, and a request goes only
to a host the manifest declares, so neither does the secret. It keeps each program's
secrets in `secrets.toml` under its root, readable by its owner alone, and only it
writes the file: they are set through its routes, as on Cloud.

- `GET /programs/{name}/secrets` lists them by name; a value is never read back.
- `PUT /programs/{name}/secrets/{secret}` with `{"value": "…"}` sets one, and answers
  `409` while it is set: a set secret is changed by revoking it and setting it again.
- `DELETE /programs/{name}/secrets/{secret}` revokes one.

## What a deployed program can reach

Everything in [the guest API](./guest-api.md): its keys, a database, logs, the clock, wakes, HTTP to the hosts its manifest declares, randomness, itself, and the programs deployed beside it that its manifest declares in `programs`. The local host registers no files and no commands. `Manifest::from_image` reads what an image claims without compiling or running it.

## MCP

- Every deployed program appears on one endpoint at `/mcp`, over Streamable
  HTTP, its tools named `{program}.{tool}`. A program name carries no dot, so
  dispatch splits on the first one.
- A call over MCP carries no caller, so the endpoint lists and calls only
  `anonymous` tools.
- A tool's budget is in its `_meta.terms`, and a query tool is marked
  read-only.
- Each program with a [README](./manifest.md#package) publishes it as the
  resource `berm://{program}/readme`.
- Deploying and undeploying emit `notifications/tools/list_changed`.
