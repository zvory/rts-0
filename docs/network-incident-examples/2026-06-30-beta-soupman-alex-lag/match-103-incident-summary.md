# Network Incident Summary

Generated: 2026-06-30T00:51:01.908Z
Input rows: 317

## Match 103

- Sources: 103
- Match run id: alex-s-lobby-1782778605186-000004
- Participants: soupman, alex
- Duration: 1480681 ms / 44849 ticks
- Rows: 297 client reports, 18 tick, 0 snapshot, 0 writer
- Transport diagnostics: WebSocket compression none=297; WebSocket extensions (empty)=297; snapshot byte source messagepack-application-payload=297; snapshot codec messagepack-compact=297; snapshot codec version 1=297; snapshot frame kind binary=297

| player | reports | primary issues | RTT max | snapshot gap max | jitter max | payload max | payload p95 | over budget | parse/decode/apply max | frame gap max | frame work max | renderer max | FPS min | cmds/burst | cmd response max | server outbound | server tick max | server lag max |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- | ---: | --- | ---: | ---: |
| 7 | 148 | command_upload_delay=73, prediction_correction=56, command_server_queue=10, command_density=7, command_response_delay=2 | 807 | 933 | 300 | 6895 | 8192 | 100% | 5/9/8 | 150 | 15 | 13 | 37 | 32/19 | 890 | 11/11/4/0 | 12 | 1 |
| 9 | 149 | command_upload_delay=68, packet_budget_pressure=37, command_server_queue=32, snapshot_gap=9, command_density=2, command_response_delay=1 | 1385 | 1318 | 684 | 8005 | 8192 | 100% | 12/19/1 | 58 | 49 | 47 | 94 | 41/16 | 1533 | 6/6/9/0 | 10 | 2 |

### Classification
- server tick/scheduler pressure: indicated: player 7 slow_tick_count max 32; player 9 slow_tick_count max 32; serverTick tick_ms max 303
- server snapshot projection/compact/serialization cost: not indicated
- WebSocket writer/send and outbound snapshot pressure: indicated: player 7 server_reliable_drained_before_snapshot_max max 4; player 9 server_reliable_drained_before_snapshot_max max 3
- client network RTT/jitter/snapshot delivery gaps: indicated: player 7 rtt_ms max 544; player 7 rtt_max_ms max 807; player 7 snapshot_jitter_ms max 300; player 7 snapshot_gap_max_ms max 933
- browser payload parsing/decode/apply/frame work: indicated: player 7 snapshot_bytes_p95 max 8192; player 7 snapshot_over_segment_budget_pct_x100 max 10000; player 7 frame_gap_max_ms max 150; player 9 snapshot_bytes_p95 max 8192
- command density and receipt volume: indicated: player 7 commands_issued max 32; player 7 command_burst_max max 19; player 7 server_command_receipts_accepted max 32; player 9 commands_issued max 41
- command upload/receipt/sim/downstream/render delay: indicated: player 7 acknowledged_command_latency_ms max 654; player 7 command_issue_to_server_receipt_max_ms max 890; player 7 command_server_receipt_to_sim_ack_max_ms max 528; player 7 command_issue_to_sim_ack_max_ms max 890
- prediction disable/replay/late-snapshot coverage: indicated: player 7 prediction_replay_max_ticks max 19; player 7 snapshot_late_frame_count max 262; player 7 predicted_snapshot_late_frame_count max 262; player 9 snapshot_late_frame_count max 465
- Transport/WebTransport theory: Unsupported: Fly logs and ClientNetReport do not expose packet loss, retransmits, or per-packet browser transport data. Packet-budget fields are payload bytes only and exclude WebSocket/TLS/TCP/IP overhead. Client reports did not show negotiated WebSocket compression.
