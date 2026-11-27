---
name: crab-debug
description: Diagnose CrabTalk failures, refusals, slow calls and unexpected costs. Use explanation codes, calls, logs, replay, wakes, deploy history or usage to investigate a running service or failed deploy.
---

# Debug a service

Start from what the failure says, then narrow to one call.

## A refusal with a code

A refusal that names a code, as `Error: 402 [E0001]: …`, is explained by `crab explain E0001`: what it means, its corner cases and what to do. Run it before deciding anything or asking the person. For funding, grants, secrets or other approvals, use `crab-ship`. Every code: [Errors](https://crabtalk.ai/docs/errors/introduction).

## Find the call

| Command | What it reads |
| --- | --- |
| `crab calls <service>` | Its calls of the last 7 days, newest first: caller, outcome, duration, what each used and cost. `--tool`, `--outcome done\|failed\|denied\|refused` and `--limit` narrow them. Cloud only. |
| `crab logs <service>` | What it emitted, oldest first. `--after <n>` prints those after a sequence number. See the local loop in `crab-build` for the host's own log. |
| `crab deploys <service>` | Its last 100 deploys, refused and failed ones included, with pending approval links. An owner calling a service awaiting its first approval receives `E0046`. Cloud only. |
| `crab wakes <service>` | The wakes armed to call its tools. |
| `crab cron ls <service>` | Its cron tools with their expression and next run. |
| `crab var ls <service>` | The value or default each var holds in this deployment. |
| `crab usage <service>` | What it used each of the last 30 days, including sampled database allocation (\`—\` when no sample exists). With no service, Cloud account totals and plan usage. |
| `crab cost <service>` | What it cost in a UTC month, by resource; `--month YYYY-MM`. Cloud only. |

Cloud reads require account ownership and the matching API-key permission. See [Calls and logs](https://crabtalk.ai/docs/guides/monitoring).

## Replay a failed call

A call that failed or trapped can retain its input and a tape of outside answers for 7 days on Cloud. Successful, denied and refused calls omit replay data. A tape can end early at the recording limit.

1. `crab calls <service> --outcome failed` finds its id.
2. `crab replay <service> <id>` runs it again in the CLI's own process, answered from the tape without live services, printing its logs. It exits `2` when it fails again.
3. Fix the code, then from the service's crate run `crab replay <service> <id> --build`: it builds the crate, prints the recorded and rebuilt image digests, and replays the same input and tape against the new image. A request that differs from the tape stops the replay, naming the syscall where the code took another path. Exit `0` means the call now succeeds.
4. `crab replay <service> <id> --on default` runs the recorded call on the other context's host against its live state. Check authorization for effects before using `--on`. `--build` and `--on` exclude each other.

The tape stores response data as received, including any credentials an upstream returns: do not paste it anywhere. See [Replaying](https://crabtalk.ai/docs/guides/replay).

## Budgets and cost

- For a budget refusal, read its explanation code and follow the budget reference in `crab-build`.
- A cost surprise: `crab cost` splits a month by resource and `crab calls` shows what each call used.

## Reporting

Report what you found against the evidence: the call id, its outcome, the log lines and the code. Do not retry a failed write, workflow run or deploy until you know whether the first one took effect.
