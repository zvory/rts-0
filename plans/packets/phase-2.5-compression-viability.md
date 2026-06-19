# Phase 2.5 Compression Viability

Generated: 2026-06-19

## Stack Finding

The current live WebSocket path is Axum 0.8 `WebSocketUpgrade`, which depends on
`tokio-tungstenite` 0.29 and `tungstenite` 0.29 for WebSocket protocol handling. The checked
Tungstenite 0.29 crate does not expose a `permessage-deflate` feature and its README states that
permessage-deflate is not supported. The current server code also has no compression negotiation
configuration on the Axum upgrade path.

That means the Phase 2 offline `deflateRaw` result is still useful as a size proxy, but the current
runtime stack cannot directly turn it into negotiated WebSocket compression.

## Diagnostics Added

- Client `ClientNetReport` payloads now include `websocketExtensions`, `websocketCompression`, and
  `snapshotByteSource`.
- Server `client_net_report` structured logs now emit those fields.
- `scripts/parse-net-report-logs.mjs` now summarizes transport diagnostics and keeps packet-budget
  bytes explicitly labeled as application payload bytes.
- `scripts/client-perf-harness.mjs` now writes a top-level `websocket` block and mirrors the report
  fields in `snapshotPacketBudget`.

## Evidence Matrix

| workload | observed compression state | bytes measured | notes |
| --- | --- | --- | --- |
| Matt/Alex browser harness | `none`, empty `WebSocket.extensions` | application payload bytes | `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 6 --snapshot-codec-bakeoff` passed. Client report p95 bucket was 2048 bytes with 92.78% over the 1280-byte budget; captured compact JSON p95 was 1500 bytes and offline deflate proxy p95 was 536 bytes. |
| vehicle-wall stress browser harness | `none`, empty `WebSocket.extensions` | application payload bytes | `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6 --snapshot-codec-bakeoff` passed. Client report p95 bucket was 2048 bytes with 100% over budget; captured compact JSON p95 was 1581 bytes and offline deflate proxy p95 was 210 bytes. |
| AI/server perf harness | not browser-negotiated | serialized compact JSON bytes | `scripts/ai-perf-harness.sh --ticks 5000 --perf full --no-log-snapshots` passed. It serialized 20,000 snapshots, p95 payload 3371 bytes, max 5190 bytes, 78.64% over budget, p95 serialize 22 us, and no slow ticks at or above 33 ms. |
| beta/Fly logs | not candidate-deployed | not available for new fields | `/version` returned `e4b93fbb460f`; recent beta logs were accessible but had no matching `client_net_report`, `websocket_compression`, `snapshot_byte_source`, or writer rows. Verify again after a candidate build reaches beta. |

## Phase 2.6 Recommendation

Do not roll WebSocket compression default-on from the current stack. Phase 2.6 should be an
implementation-route follow-up if compression is still desired: either replace the WebSocket upgrade
path with one that supports `permessage-deflate`, add a safe transport wrapper, or explicitly ship an
application-level compressed snapshot envelope with compact JSON fallback and a rollback switch.

Delta snapshot work should still wait until the compression route is either implemented with
evidence or deliberately deferred.
