# 2026-06-30 beta soupman/Alex lag incident

Preserved lag artifact for the player-reported "Superman and Alex" game. The stored match-history
participant name is `soupman`; this is the Superman player from the report.

The game ran on beta, not mainline. Mainline Fly log search for the same UTC window returned zero
matching rows.

The preserved match replay is a replay artifact schema 2 payload kept for incident analysis only.
Current checkpoint-backed replay loaders reject schema 2 rather than loading or migrating it.

## Match

| field | value |
| --- | --- |
| Public match id | `103` |
| Match run id | `alex-s-lobby-1782778605186-000004` |
| Room | `alex's lobby` |
| UTC window | `2026-06-30T00:16:45.186Z` to `2026-06-30T00:41:25.868Z` |
| ET window | `2026-06-29 20:16:45` to `2026-06-29 20:41:25` |
| Duration | `1480681ms` / `44849` ticks |
| Build | `5d33dc1e4d7c` |
| Map | `Default`, hash `13ac64aab0be91a8` |
| Seed | `577023137` |
| Players | `soupman` player `7`, `alex` player `9` |
| Winner | `alex` / player `9` |

## Evidence

| file | contents |
| --- | --- |
| `match-103-runid-logs.jsonl` | Fly log rows filtered to the exact live match run id: start/end, client network reports, and slow tick summaries. |
| `match-103-replay.schema2-unsupported.json` | Historical schema 2 `ReplayArtifactV1` from `match_replays.artifact_json`; not loadable by current replay runtime. |
| `match-103-db-summary.json` | Public `/api/matches` summary row for match `103`. |
| `match-103-incident-summary.md` | Markdown output from `scripts/parse-net-report-logs.mjs`. |
| `match-103-incident-summary.json` | Structured parser output. |
| `match-103-incident-rows.tsv` | Parser per-player aggregate TSV. |
| `match-103-client-net-rows.tsv` | Extracted per-report client metrics for sorting worst windows. |
| `match-103-tick-rows.tsv` | Extracted slow server tick summaries. |
| `match-103-key-metrics.json` | Compact match, replay, and per-player metric summary. |

To regenerate the parser summary from this directory:

```bash
node ../../../scripts/parse-net-report-logs.mjs match-103-runid-logs.jsonl
```

## Diagnosis

This was a mixed lag incident: server-side pathing hitches plus client/network delivery delays.

Server evidence:

- The run had `32` slow ticks by match end.
- Preserved slow-tick log rows show `18` slow tick summaries; all exceeded the `33ms` tick budget,
  `12` exceeded `50ms`, `4` exceeded `100ms`, and `1` exceeded `200ms`.
- Worst preserved tick: `303ms` at `2026-06-30T00:40:45.721829Z`, with `297ms` in
  `awaiting_paths`.
- Slowest phase was `awaiting_paths` in `16` of `18` preserved slow-tick summaries. Snapshot
  projection was cheap (`max_snapshot_ms` `0-1ms`), and match end reported
  `max_head_of_line_count=0`.

Client/network evidence:

- `soupman` / player `7`: command response max `890ms`, p95 `616ms`; RTT max `807ms`; snapshot
  gap max `933ms`; snapshot jitter max `300ms`; frame gap max `150ms`.
- `alex` / player `9`: command response max `1533ms`, p95 `563ms`; RTT max `1385ms`; snapshot
  gap max `1318ms`; snapshot jitter max `684ms`; frame work max `49ms`.
- Both players repeatedly crossed bad thresholds: more than 50 reports per player had command
  response over `250ms`, more than 25 reports per player had snapshot gaps over `500ms`, and both
  saw many RTT-max spikes over `300ms`.
- Snapshot payload pressure was persistent: per-window snapshot p95 reached `8192` bytes for both
  players while the single-segment budget is `1280` bytes. Logs show WebSocket compression was not
  negotiated.

The worst sustained period was roughly `2026-06-30T00:21Z` through `00:28Z`, with later spikes near
`00:40Z`. Future lag work should use this artifact to investigate path request batching/backlog in
`MoveCoordinator::process_awaiting_paths`, command upload/receipt delays during command bursts, and
snapshot payload growth under late-game entity counts.
