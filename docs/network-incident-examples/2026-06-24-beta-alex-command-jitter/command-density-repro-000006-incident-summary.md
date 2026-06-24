# Command-density repro 000006 incident summary

- Match run id: `commander-s-lobby-1782262168796-000006`
- Started: `2026-06-24T00:49:28.796Z`
- Participants: `Commander`
- Build: `fb3abd0b3fcc`
- Raw log file: `command-density-repro-000006-logs.jsonl`

## Important row

- Timestamp: `2026-06-24T00:49:39.057Z`
- Player: `11`
- `primary_issue=command_server_queue`
- `commands_issued=25`
- `command_server_received=25`
- `command_sim_acknowledged=25`
- `rtt_ms=52`
- `rtt_max_ms=68`
- `snapshot_jitter_ms=5`
- `snapshot_gap_max_ms=89`
- `jitter_samples=55`
- `snapshot_burst_max=8`
- `frame_gap_max_ms=108`
- `command_server_receipt_to_sim_ack_max_ms=70`
- `command_issue_to_sim_ack_max_ms=84`
- `server_tick_ms=2`
- `server_lag_ms=1`
- `slow_tick_count=0`
- `head_of_line_count=0`
- `prediction_mode=tracking`
- `correction_distance_px=0`
- `correction_count=0`

## Second row

- Timestamp: `2026-06-24T00:50:09.160Z`
- `commands_issued=4`
- `snapshot_burst_max=85`
- `frame_gap_max_ms=103`
- `command_server_receipt_to_sim_ack_max_ms=87`
- `command_issue_to_sim_ack_max_ms=95`
- `server_tick_ms=3`
- `server_lag_ms=0`

## Interpretation

This run is the clearest command-density reproduction. RTT and snapshot jitter were low in the
important row, prediction stayed in `tracking`, and corrections stayed at zero, while command
receipt-to-sim-ack latency still reached `70ms` under dense command issue.
