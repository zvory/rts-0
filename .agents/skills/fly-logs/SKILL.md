---
name: fly-logs
description: Check Fly.io beta or mainline server logs for this RTS project. Use when debugging deployed behavior, post-deploy regressions, WebSocket or lobby failures, match-history recording, server crashes, performance spikes, or anything that may differ between local, beta, and production.
---

# Fly Logs

Use this skill to inspect deployed server behavior on Fly.io.

## When To Use

- Post-deploy regressions or reports that beta/mainline behaves differently than local.
- WebSocket, lobby, room lifecycle, match start/end, or disconnect failures on deployed apps.
- Match-history recording or `/api/matches` behavior on deployed apps.
- Server crashes, restarts, slow ticks, performance spikes, or missing expected server events.
- Any investigation where deployed environment variables, machine restarts, or production traffic could explain the issue.

## Commands

From the repo root:

```bash
scripts/fly-logs.sh beta recent
scripts/fly-logs.sh mainline recent
```

For live tailing, always bound the command so it cannot stream forever:

```bash
timeout 30 scripts/fly-logs.sh beta tail
timeout 30 scripts/fly-logs.sh mainline tail
```

The helper reads `FLY_API_TOKEN` from the shell environment, this worktree's `.env`, or the main
worktree's `.env`. The real token is a local secret and must never be committed, printed, or pasted
into issue/PR text.

## How To Read Output

- Logs are JSON. Use `rg` for quick filtering.
- Useful patterns include `error`, `panic`, `database connected`, `match recorded`,
  `RTS_RECORD_MATCHES`, `performance tick summary`, `WebSocket`, `room`, and `lobby`.
- Finished matches use historical `search`; live checks use `recent`.

## Report Back

Summarize the relevant log lines in plain language. Do not include secrets or large raw log dumps.
If logs were unavailable because `FLY_API_TOKEN` is missing or expired, say that and continue with
local evidence.
