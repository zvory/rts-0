# Phase 2.5 - MessagePack Snapshot Frames

## Phase Status

- [x] Done.

## Objective

Replace live snapshot text frames with MessagePack binary frames over the existing WebSocket. This
phase should ship the first real binary snapshot codec, keep reliable non-snapshot messages as JSON
text, preserve the semantic `Snapshot` / `GameState.applySnapshot` boundary, and prove locally that
normal play still works with lower snapshot payload bytes.

## Background

Phase 2 showed that schema-less MessagePack over the compact snapshot object cut deterministic
fixture p95 bytes from 17,533 to 8,826. That does not reach the single-segment target for large full
snapshots, but it roughly halves full-snapshot payload pressure without swapping the WebSocket stack
or depending on `permessage-deflate` negotiation.

The game is pre-alpha, so stale browser clients and compatibility migrations are out of scope for
this rollout. Do not spend implementation budget on long-running fallback modes, client capability
negotiation, or preserving compact JSON for old clients. If the MessagePack path fails local or beta
smoke, the rollback is to revert the MessagePack change rather than carry both snapshot formats.

## Work

- Implement MessagePack snapshot encoding:
  - add a `messagepack-compact` snapshot codec/version to the Rust and JS protocol mirrors;
  - encode the existing compact snapshot shape as MessagePack, preferably reusing the current
    `CompactSnapshot` serializer so semantic snapshot construction remains unchanged;
  - send snapshots as WebSocket binary frames and keep reliable messages (`welcome`, `start`,
    `lobby`, `pong`, errors, lab/replay control messages) as JSON text;
  - add a small explicit binary-frame discriminator or header if needed so malformed/unknown binary
    frames fail clearly instead of being guessed by decoder heuristics;
  - decode MessagePack in the browser into the same raw compact snapshot object that
    `decodeCompactSnapshot` already expands into the semantic snapshot shape.
- Keep the rollout direct:
  - make MessagePack the normal snapshot path once the local tests and smoke pass;
  - do not add compact JSON compatibility fallback, stale-client negotiation, or long-lived
    runtime selection;
  - do not reopen WebSocket compression, CBOR, protobuf, or custom binary unless MessagePack proves
    unworkable and the user explicitly changes direction.
- Finalize diagnostics:
  - report/log the active snapshot codec, codec version, and frame kind (`binary`);
  - label snapshot byte metrics as MessagePack payload bytes before WebSocket/TLS/TCP overhead;
  - keep packet-budget p95, max, over-budget count/rate, parse/decode/apply p95, server serialize
    p95, writer send timing, snapshot gaps/jitter, and command acknowledgement fields clear enough
    to compare with Phase 1/2 compact JSON baselines.
- Verify important runtime surfaces locally:
  - normal live match with fog enabled;
  - commands and command acknowledgement;
  - replay entry and seek if practical;
  - spectator/observer view if practical;
  - lab/dev-watch path if practical;
  - reconnect/page refresh behavior with the current deployed client code, not stale cached code.
- Update docs and tests:
  - update `docs/design/protocol.md` for MessagePack binary snapshot frames and the lack of
    backwards-compatibility fallback;
  - update `docs/perf-tracing.md` and parser/harness language so byte metrics are not described as
    compact JSON or compressed wire bytes;
  - update focused protocol/client/report tests for binary frame parsing, codec constants, malformed
    binary rejection, and report/log/parser fields.

## Expected Touch Points

- `server/Cargo.toml` and `server/Cargo.lock` if a MessagePack crate is added
- `server/crates/protocol/src/lib.rs`
- `server/src/main.rs`
- `server/src/perf.rs` or writer timing logs if codec labels are added there
- `server/src/structured_log.rs`
- `client/src/protocol.js`
- `client/src/net.js`
- `client/src/client_perf_report.js`
- `scripts/client-perf-harness.mjs`
- `scripts/parse-net-report-logs.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/client_net_report_fields.mjs`
- `tests/net_report_log_parser.mjs`
- `tests/protocol_parity.mjs`
- focused Rust protocol tests for MessagePack encoding and malformed frame handling

## Implementation Checklist

- [x] Confirm the archived compression-viability evidence is superseded by the MessagePack direction.
- [x] Add Rust MessagePack encoding for compact snapshots.
- [x] Add browser MessagePack decoding for snapshot binary frames.
- [x] Make MessagePack binary frames the normal live snapshot path after local validation.
- [x] Keep reliable non-snapshot messages as JSON text.
- [x] Add codec/version/frame-kind report, log, parser, and harness fields.
- [x] Remove or bypass stale WebSocket compression diagnostics planned for the old Phase 2.5.
- [x] Update protocol/perf docs and focused tests.
- [x] Run local realistic benchmarks and record compact JSON baseline vs MessagePack results where
      practical.
- [x] Mark this phase as done in this file.

## Implementation Notes

- MessagePack dependency choice: no new Rust or browser dependency. The server uses a small
  in-repo MessagePack writer over the existing `CompactSnapshot` serializer; the browser uses a
  dependency-free MessagePack reader in `client/src/protocol.js`.
- Frame shape: binary snapshots start with `RTSM`, one byte snapshot codec version `1`, then a
  MessagePack map containing the compact snapshot object (`t: "snapshot"`, `v: 22`, and the
  existing compact slots).
- Default path: `messagepack-compact` binary snapshots are the normal live snapshot frame; reliable
  non-snapshot messages remain JSON text. Compact JSON remains only as a local/historical baseline.
- Local payload evidence: deterministic fixture bake-off reported compact JSON p95 17,533 bytes vs
  MessagePack p95 8,826 bytes. The AI perf harness reported 20,000 MessagePack snapshots with avg
  1,096 bytes, p95 1,714 bytes, max 3,194 bytes.
- Local live smoke note: starting a release server for `tests/server_integration.mjs` was attempted
  with `RTS_ADDR=127.0.0.1:18081`, but the sandbox denied binding the socket with
  `Operation not permitted (os error 1)`, so live browser/server smoke remains manual.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/client_net_report_fields.mjs`
- `node tests/net_report_log_parser.mjs`
- `node tests/protocol_parity.mjs`
- focused Rust protocol tests for the MessagePack codec
- a focused local live/browser smoke for normal match snapshots and commands
- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6` when practical;
  use a fresh schema 3 replay workload if replay playback evidence is required
- a high-entity stress browser harness workload when practical
- `scripts/ai-perf-harness.sh --ticks 5000 --perf full --no-log-snapshots` or an equivalent
  documented server-side payload benchmark
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If the local live/browser smoke fails, fix or revert within this phase. Do not add a compatibility
fallback as the workaround.

## Manual Test Focus

Run a normal local match with the MessagePack path active. Confirm snapshots flow, commands
acknowledge, fog still looks correct, replay/lab/dev-watch entry still works where checked, and
diagnostics clearly state `messagepack-compact` with lower snapshot payload bytes than compact JSON
baselines.

## Handoff Expectations

State the MessagePack dependency choice, final codec/version/header shape, local before/after payload
and timing numbers, and any runtime surface not manually checked. If MessagePack is not shippable,
say that plainly and recommend reverting or reopening the binary-codec decision rather than carrying
both compact JSON and MessagePack.
