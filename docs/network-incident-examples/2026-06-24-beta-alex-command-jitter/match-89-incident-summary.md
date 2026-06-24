# Match 89 incident summary

- Public match id: `89`
- Match run id: `alex-s-lobby-1782260675313-000003`
- Participants: `alex`, `alex`
- Started: `2026-06-24T00:24:35.313112Z`
- Ended: `2026-06-24T00:25:48.617844Z`
- Duration: `73304ms` / `2221` ticks
- Build: `fb3abd0b3fcc`
- Map: `Default`, hash `13ac64aab0be91a8`
- Replay: `match-89-replay.json`

## Metrics

Player `4` had `3` reports. Maxima from the exact run logs:

- `rtt_ms=58`
- `rtt_max_ms=17`
- `snapshot_jitter_ms=109`
- `snapshot_gap_max_ms=317`
- `frame_gap_max_ms=9`
- `acknowledged_command_latency_ms=118`
- `server_tick_ms=6`
- `server_lag_ms=1`
- `slow_tick_count=0`
- `head_of_line_count=0`

Player `5` had one end-row with `rtt_ms=157`, `server_tick_ms=6`, and `server_lag_ms=1`.

## Interpretation

This recorded match shows client-observed snapshot jitter/gap evidence and one high RTT end-row, but
does not show server tick pressure or writer head-of-line pressure in the preserved rows.
