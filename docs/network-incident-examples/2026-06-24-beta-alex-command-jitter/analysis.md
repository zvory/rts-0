# Analysis

This is an evidence package, not a gameplay change.

## Main finding

The available telemetry points away from server tick/scheduler lag. Across these cases, the reported
server-side health fields stayed small: `server_tick_ms` reached at most `6`, `server_lag_ms` stayed
at `0-1`, `slow_tick_count` stayed `0`, and `head_of_line_count` stayed `0`.

The player-visible symptoms line up better with command-density/cadence and client pacing:

- The aborted Alex run had the strongest bad row: at `2026-06-24T00:24:03.433Z`, player `2` was
  active and focused, `primary_issue=command_upload_delay`, `frame_gap_max_ms=5734`,
  `command_issue_to_sim_ack_max_ms=347`, while `server_tick_ms=3` and `server_lag_ms=1`.
- The same aborted run also had player `2` maxima of `rtt_ms=239`, `rtt_max_ms=370`,
  `snapshot_gap_max_ms=1156`, `command_issue_to_server_receipt_max_ms=414`,
  `command_server_receipt_to_sim_ack_max_ms=406`, and `command_issue_to_sim_ack_max_ms=429`.
- The later command-density repro shows a cleaner command-path signal: `25` commands issued,
  server-received, and sim-acknowledged in the `2026-06-24T00:49:39.057Z` row, with
  `primary_issue=command_server_queue`, `command_server_receipt_to_sim_ack_max_ms=70`,
  `command_issue_to_sim_ack_max_ms=84`, `server_tick_ms=2`, `server_lag_ms=1`, and no prediction
  corrections.
- Match `89` itself has milder but still relevant client-side delivery evidence. Player `4` reported
  `snapshot_jitter_ms=109`, `snapshot_gap_max_ms=317`, and
  `acknowledged_command_latency_ms=118`, while server tick and lag stayed healthy.

## Case interpretation

### Match 89

Public match `89` is useful because it has a stored replay and match-history row. It confirms the
build, map, participants, duration, and run id for the recorded Alex vs Alex game:
`alex-s-lobby-1782260675313-000003` on build `fb3abd0b3fcc`, map `Default`, map hash
`13ac64aab0be91a8`, duration `73304ms` / `2221` ticks.

The exact run-id logs contain four client reports and a match-ended row. Player `4` reported
snapshot jitter and gap spikes; player `5` had one end-row with `rtt_ms=157`. This recorded match
does not show server lag or writer head-of-line pressure.

### Aborted run 000002

The run immediately before match `89` is a stronger reproduction of the reported feel, but it did
not produce match history because the room was disposed before a recorded match ended. The raw log
file includes one adjacent Commander row from `commander-s-lobby-1782260545876-000001`; the summaries
for this case focus on `alex-s-lobby-1782260563198-000002`.

The target Alex run shows both command-path delay and frame stalls. The worst focused row has a
multi-second frame gap and hundreds of milliseconds of command issue-to-ack delay while server
health remains normal. That makes this the best evidence in this package for "commands feel bad even
when the server loop is not overloaded."

### Command-density repro 000006

The later Commander solo run isolates command density. The first report row has low-ish RTT
(`rtt_ms=52`, `rtt_max_ms=68`), low snapshot jitter (`5ms`), and no prediction corrections, but it
still classifies as `command_server_queue` with `25` issued/received/acked commands and `70ms`
server-receipt-to-sim-ack max. The second row has only `4` commands issued but a
`snapshot_burst_max=85` and `87ms` server-receipt-to-sim-ack max.

This supports looking at command batching/cadence and snapshot burst behavior before treating the
incident as generic network lag.

## HUD `jit`

The HUD `jit` label is snapshot arrival jitter from `MatchHealth`, surfaced as `snapshot_jitter_ms`
in `ClientNetReport`. It does not refer to JavaScript JIT compilation. A rising `jit` number during
high command density means the client is observing irregular snapshot timing, not that the JS engine
is compiling code.

## Limits

Fly logs and `ClientNetReport` do not expose packet loss, retransmits, or per-packet browser
transport data. Packet-budget fields count application payload bytes, not full WebSocket/TLS/TCP/IP
wire bytes. The absence of server lag in these rows is evidence against server-loop blame for these
windows, not proof that every deployed host resource was perfect for every instant.

No replay exists for the aborted Alex run in this package because no match-history row was recorded.
The command-density repro is a solo live sandbox and is included for telemetry shape, not match
outcome review.
