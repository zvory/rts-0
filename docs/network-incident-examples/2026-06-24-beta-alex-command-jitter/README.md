# 2026-06-24 beta Alex command jitter evidence

This directory preserves evidence for a beta player-visible command/jitter incident on build
`fb3abd0b3fcc`. It covers the recorded public Alex vs Alex match, the immediately preceding
aborted Alex lobby run, and a later solo command-density reproduction.

The main point of the package is to keep raw evidence and a short interpretation together. The
available rows do not support a server-lag explanation: reported `server_tick_ms`, `server_lag_ms`,
`slow_tick_count`, and `head_of_line_count` stayed healthy. The stronger signals are command-path
delay, snapshot arrival cadence/jitter, and client frame stalls.

## Evidence

| file | contents |
| --- | --- |
| `match-89-db-summary.json` | Public match-history summary for match `89` without the replay blob. |
| `match-89-replay.json` | Stored replay artifact for public match `89`. |
| `match-89-runid-logs.jsonl` | Exact Fly JSONL rows for `alex-s-lobby-1782260675313-000003`. |
| `match-89-incident-summary.md` | Concise facts for match `89`. |
| `match-89-incident-summary.json` | Machine-readable facts for match `89`. |
| `aborted-run-000002-logs.jsonl` | Exact Fly JSONL window for the pre-recording aborted run. Includes one adjacent Commander row before the target Alex run. |
| `aborted-run-000002-incident-summary.md` | Concise facts for `alex-s-lobby-1782260563198-000002`. |
| `aborted-run-000002-incident-summary.json` | Machine-readable facts for the aborted run. |
| `command-density-repro-000006-logs.jsonl` | Exact Fly JSONL rows for `commander-s-lobby-1782262168796-000006`. |
| `command-density-repro-000006-incident-summary.md` | Concise facts for the command-density reproduction. |
| `command-density-repro-000006-incident-summary.json` | Machine-readable facts for the command-density reproduction. |
| `analysis.md` | Interpretation across the three cases. |
| `player-report-quotes.md` | User-supplied symptom phrases and report context. |

Fly log rows are JSONL. The `.message` fields include ANSI tracing escapes from server logs. Strip
them before ad hoc text analysis, for example:

```bash
jq -r '.message' match-89-runid-logs.jsonl | perl -pe 's/\e\[[0-9;]*m//g'
```

The standard parser can still be run directly on these raw logs:

```bash
node ../../../scripts/parse-net-report-logs.mjs \
  match-89-runid-logs.jsonl \
  aborted-run-000002-logs.jsonl \
  command-density-repro-000006-logs.jsonl
```

## Cases

| case | UTC window | identifier | replay/history |
| --- | --- | --- | --- |
| Recorded Alex vs Alex | `2026-06-24T00:24:35.313112Z` to `2026-06-24T00:25:48.617844Z` | public match `89`, run `alex-s-lobby-1782260675313-000003` | replay stored in `match-89-replay.json` |
| Aborted Alex lobby | approx `2026-06-24T00:22:30Z` to `2026-06-24T00:24:24Z` | run `alex-s-lobby-1782260563198-000002` | no match-history replay; room disposed at `2026-06-24T00:24:24Z` |
| Command-density repro | started `2026-06-24T00:49:28.796Z` | run `commander-s-lobby-1782262168796-000006` | solo live sandbox row, no replay artifact in this package |

## HUD label note

The player-facing `jit` indicator means snapshot arrival jitter measured by the client. It is not
JavaScript compiler JIT behavior. In this evidence, high `jit` should be read as irregular snapshot
cadence at the client.
