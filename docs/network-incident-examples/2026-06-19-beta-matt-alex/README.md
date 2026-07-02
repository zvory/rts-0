# 2026-06-19 beta Matt/Alex lag incident

This is a preserved example of a real player-visible lag report where the server stayed healthy
but one player's client and network path made the match feel unplayable.

The games happened on beta during the evening of 2026-06-18 America/New_York, recorded in UTC on
2026-06-19. The current beta deploy changed after the matches were recorded, so the public match
API may warn that the replays are potentially incompatible with the current server. The raw replay
artifacts in this directory are the stored DB artifacts from the recording build. They are replay
artifact schema 2 payloads preserved for incident analysis only; current checkpoint-backed replay
loaders reject them as unsupported.

## Evidence

| file | contents |
| --- | --- |
| `fly-match-54-all.jsonl` | All Fly log rows returned for match 54's UTC interval. |
| `fly-match-55-all.jsonl` | All Fly log rows returned for match 55's UTC interval. |
| `match-54-replay.schema2-unsupported.json` | Historical schema 2 replay artifact for match 54; not loadable by current replay runtime. |
| `match-55-replay.schema2-unsupported.json` | Historical schema 2 replay artifact for match 55; not loadable by current replay runtime. |
| `matches-54-55-db-summary.json` | DB match rows without the large replay blobs. |
| `parsed-net-reports.tsv` | Parsed client network reports used for this analysis. |
| `player-report-quotes.md` | Exact user-supplied quotes about the reported player experience. |
| `analysis.md` | Analysis-only interpretation of the evidence and player report. |

Fly log rows are JSONL. The app log text inside `.message` still contains ANSI escape sequences
from tracing output. Strip them before ad hoc text analysis, for example:

```bash
jq -r '.message' fly-match-54-all.jsonl | perl -pe 's/\e\[[0-9;]*m//g'
```

For the repeatable parser/playbook path, run:

```bash
node ../../../scripts/parse-net-report-logs.mjs fly-match-54-all.jsonl fly-match-55-all.jsonl
```

The parser emits the compact table and evidence-bounded classification described in
[`docs/perf-tracing.md`](../../perf-tracing.md#network-incident-parser). It reports the newer
payload, packet-budget, command milestone, and server snapshot timing fields as unavailable for
these older logs instead of treating them as zero.

## Matches

| match | UTC window | winner | players | replay build |
| --- | --- | --- | --- | --- |
| 54 | 2026-06-19 00:54:23 to 01:00:57 | alex | alex=`4`, Matt=`5` (`<b>matt</b>` in stored name) | `7e5fb0881792` |
| 55 | 2026-06-19 01:06:06 to 01:11:05 | matt | alex=`7`, Matt=`8` | `7e5fb0881792` |

The current beta server at the time of this write-up was `ca1a92548ac8`, after a deploy moved past
the replay recording build.

## Conclusion

This was not a server-lag incident. It was a client/network-path incident for Matt.

Matt had both:

- low and bursty local frame cadence, making the client feel sluggish;
- poor network/snapshot timing, making command response and authoritative confirmation feel late.

The server stayed inside budget:

- `server_tick_ms` from client reports stayed at `3-6ms`;
- `server_lag_ms` stayed at `1ms`;
- `slow_tick_count` stayed `0`;
- `head_of_line_count` stayed `0`;
- match wall time matched simulation time: match 54's `11914` ticks is within 15 ms of the stored
  duration at 33 ms/tick, and match 55's `9054` ticks is within 14 ms.

Matt's telemetry was bad enough to explain an unplayable feel. In match 54, his notable reports
showed:

- RTT range `89-292ms`, with report-window max RTT up to `368ms`;
- snapshot jitter up to `104ms`;
- snapshot gaps up to `516ms`;
- frame gaps up to `309ms`;
- FPS estimate mostly `17-23`;
- command acknowledgement latency up to `243ms`.

Alex in the same match was much healthier:

- RTT range `12-35ms`, with report-window max RTT up to `63ms`;
- FPS estimate `66-84`;
- the same server tick and scheduler health values.

Match 55 had fewer notable reports in the log window but the same shape:

- Alex: `22ms` RTT, `11ms` jitter, `63ms` max snapshot gap, `83fps`.
- Matt: `102ms` RTT, `59ms` jitter, `344ms` max snapshot gap, `23fps`.

## Prediction interpretation

The player report said movement and progress bars did not visibly stutter, even though command
response felt bad. That is consistent with the telemetry.

Prediction was still in `tracking` mode during the bad windows, and correction count/distance
stayed `0`. That means client-side prediction was masking continuous movement/progress smoothly
enough for Matt not to notice movement stutter. It does not mean commands were responsive: command
acknowledgement and authoritative effects still had to pass through Matt's RTT, bursty snapshot
arrival, and local frame stalls.

One logging nuance: `primary_issue="prediction_disabled"` is not sufficient by itself. The
`predictionDisableCount` field is cumulative, so after one prediction disable event, later notable
reports may still be classified that way even when `predictionMode=tracking`. For this incident,
use the raw fields (`rtt_ms`, `rtt_max_ms`, `snapshot_jitter_ms`, `snapshot_gap_max_ms`,
`frame_gap_max_ms`, `fps_estimate`, `acknowledged_command_latency_ms`, `server_tick_ms`,
`server_lag_ms`, `slow_tick_count`, and `head_of_line_count`) rather than the primary issue label
alone.

## Diagnostic pattern

Use this incident as the canonical example for separating three different lag classes:

1. Server lag: high `server_tick_ms`, high `server_lag_ms`, increasing `slow_tick_count`, or
   head-of-line/backlog. Not present here.
2. Network or snapshot delivery lag: high RTT, high RTT max, many bad RTT samples, large snapshot
   gaps, high snapshot jitter, and delayed command acknowledgement. Present for Matt.
3. Local client frame pacing: low FPS estimates and large frame gaps. Present for Matt and likely
   amplified the subjective lag.

When movement looks smooth but commands feel delayed, check prediction state and correction metrics
before assuming the server is smooth. Prediction can hide continuous-motion stutter while command
acks still feel slow.
