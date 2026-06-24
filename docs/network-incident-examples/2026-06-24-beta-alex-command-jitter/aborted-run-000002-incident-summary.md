# Aborted run 000002 incident summary

- Match run id: `alex-s-lobby-1782260563198-000002`
- Approx window: `2026-06-24T00:22:30Z` to `2026-06-24T00:24:24Z`
- Room disposed: `2026-06-24T00:24:24Z`
- Replay/history: none; no match-history replay was recorded for this aborted room
- Raw log file: `aborted-run-000002-logs.jsonl`

The raw log file also contains one adjacent Commander row from
`commander-s-lobby-1782260545876-000001` before the target Alex run. The metrics below summarize
only `alex-s-lobby-1782260563198-000002`.

## Metrics

Player `2` had `7` reports. Maxima:

- `rtt_ms=239`
- `rtt_max_ms=370`
- `snapshot_jitter_ms=41`
- `snapshot_gap_max_ms=1156`
- `frame_gap_max_ms=5734`
- `command_issue_to_sim_ack_max_ms=429`
- `command_issue_to_server_receipt_max_ms=414`
- `command_server_receipt_to_sim_ack_max_ms=406`
- `server_tick_ms=3`
- `server_lag_ms=1`
- `slow_tick_count=0`
- `head_of_line_count=0`

Player `3` had one report:

- `rtt_max_ms=126`
- `snapshot_jitter_ms=33`
- `snapshot_gap_max_ms=322`
- `frame_gap_max_ms=1749`
- `server_tick_ms=4`
- `server_lag_ms=1`

## Worst active/focused row

- Timestamp: `2026-06-24T00:24:03.433Z`
- Player: `2`
- `primary_issue=command_upload_delay`
- `frame_gap_max_ms=5734`
- `rtt_ms=31`
- `rtt_max_ms=108`
- `snapshot_jitter_ms=41`
- `snapshot_gap_max_ms=456`
- `command_issue_to_sim_ack_max_ms=347`
- `hidden=false`
- `focused=true`
- `server_tick_ms=3`
- `server_lag_ms=1`

## Interpretation

This is the strongest preserved evidence for the report. It combines command-path delay, snapshot
cadence issues, and a large focused client frame stall while the server tick, scheduler lag, slow
tick, and head-of-line fields remain healthy.
