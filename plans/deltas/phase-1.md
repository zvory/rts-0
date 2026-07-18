# Phase 1 - Delta Stream Foundation

## Phase Status

- [ ] Ready for implementation.

## Objective

Establish the expensive-to-reverse stateful transport boundary before implementing real delta
sections. Every snapshot sent in this phase remains a complete keyframe, but it travels through a
versioned envelope, a writer-owned per-connection baseline, and a client reconstructor that returns
the same semantic `Snapshot` consumed today.

## Design Constraints

- `LatestSnapshotSlot` continues to store the newest full projected snapshot; never store pending
  deltas.
- The writer alone owns sequence allocation, the last successfully sent baseline, reset generation,
  and periodic-keyframe cadence.
- Prepare an encoded frame and candidate baseline, send the WebSocket frame, and commit the sequence
  and baseline only after `sink.send(...)` succeeds. Serialization or send failure must not advance
  either.
- Keep reliable-message priority and all delta/keyframe encoding outside the room task.
- Events and resource remaining updates remain frame-local even though a full semantic snapshot is
  retained as comparison input.
- The client validates and reconstructs below `Net` dispatch; existing snapshot consumers remain
  unchanged.
- Keep the current WebSocket and MessagePack header. Do not add a keyframe request, acknowledgement,
  compression, negotiation, or fallback protocol.

## Envelope Contract

Bump the compact snapshot version current at implementation time and add:

```text
keyframe: { t:"snapshot", v, m:0, q:<sequence>, <current compact snapshot body> }
delta:    { t:"snapshot", v, m:1, q:<sequence>, b:<baseSequence>, d:{...}, ev?, n }
```

- `m` is keyframe or delta mode.
- `q` is a nonzero per-connection sequence assigned in successful-send order.
- `b` is required for a delta and names the exact client baseline it extends.
- `d` is reserved for Phase 2 absolute-value patches.
- A keyframe is independent of all preceding frames and replaces the reconstruction baseline.
- Do not wrap a sequence into a value that can be confused with an old baseline; close/restart the
  stream on practically unreachable exhaustion.

## Work

- Add a writer-local snapshot stream encoder containing:
  - next sequence and last successfully sent sequence;
  - last successfully sent recipient-projected semantic snapshot;
  - reset generation and bounded reset reason;
  - successful sends since the last keyframe.
- Emit a periodic keyframe every 60 successfully sent snapshots, approximately two seconds at the
  normal cadence. Base cadence on sent frames, not simulation ticks or projected snapshots.
- Split snapshot preparation/commit from generic reliable-message sending so payload diagnostics and
  timing continue to describe the actual encoded frame.
- Prove pending replacement safety: when ticks 101 through 104 replace one another, a later tick 105
  remains compared with the snapshot actually sent before them rather than any overwritten state.
- Add one shared reset-generation mechanism between the connection/lobby side and writer. Audit and
  force the next keyframe for:
  - new connection and successful `start` transition;
  - every existing pending-snapshot clear;
  - replay seek or vision-selection change;
  - Lab seek, import, reset, or accepted vision change;
  - branch promotion/start;
  - match teardown/rematch;
  - any other observer-view or projection-policy replacement found during the audit.
- Record bounded reset reasons such as `newConnection`, `start`, `seek`, `vision`, `branch`,
  `matchReset`, `periodic`, and `schema` for tests and later diagnostics.
- Add a client-owned snapshot reconstructor that:
  - validates version, mode, `q`, and `b`;
  - fully validates a keyframe before replacing its baseline;
  - rejects unsupported deltas or baseline mismatches without partial mutation;
  - resets with the connection lifecycle and before applying a new `start`;
  - returns only a complete semantic snapshot to current subscribers.
- Emit keyframes only. Server and client recognize the envelope but Phase 1 does not define any
  real section patch.
- Preserve exact `f32` simulation values with MessagePack float32 encoding when the compact numeric
  value round-trips exactly through `f32`; retain integer compact encoding and float64 for values that
  are not exact `f32`. Add browser support for the MessagePack float32 tag and prove byte-level plus
  semantic round trips.
- Update `docs/design/protocol.md` with envelope ownership, sequencing, commit-after-send, reset
  triggers, periodic cadence, event exclusion, numeric encoding, and direct rollout rules. Update
  `docs/perf-tracing.md` only if current reporting gains new counters.

## Expected Touch Points

- `server/src/connection_writer.rs`
- `server/src/lobby/connection.rs`
- `server/src/lobby/launch.rs`
- relevant reset seams under `server/src/lobby/room_task/`
- `server/crates/protocol/src/contract_metadata.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/protocol/src/messagepack_frame.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/net.js`
- `client/src/protocol_constants.js`
- `client/src/protocol_frame.js`
- `client/src/protocol_snapshot.js`
- a focused protocol-internal reconstructor module such as `client/src/snapshot_reconstructor.js`
- `docs/design/protocol.md`
- focused Rust writer/protocol tests and JS protocol/reconstructor tests
- `tests/protocol_parity.mjs`
- `tests/client_contracts.mjs`

## Implementation Checklist

- [ ] Define and document the keyframe/delta envelope and next compact version.
- [ ] Add writer-owned sequences, sent baseline, reset generation, and 60-send keyframe cadence.
- [ ] Commit sequences/baselines only after a successful WebSocket send.
- [ ] Keep the pending slot full-state and prove replaced snapshots do not advance the baseline.
- [ ] Audit every lifecycle/projection reset and force the next keyframe.
- [ ] Add the client reconstruction boundary and full-keyframe validation.
- [ ] Preserve exact `f32` values with MessagePack float32 support.
- [ ] Keep all existing semantic snapshot consumers unchanged.
- [ ] Update protocol/perf documentation and focused tests.
- [ ] Mark this phase done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol`
- Focused `rts-server` connection-writer/lobby tests covering commit-after-send, pending replacement,
  reset reasons, and successful-send keyframe cadence.
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- Focused client reconstructor tests for atomic keyframe replacement, stale/duplicate sequences,
  reset, unsupported deltas, and float32 frames.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Run a normal match long enough to exercise ordinary snapshot replacement and confirm gameplay, fog,
interpolation, selection, prediction acknowledgements, and transient effects are unchanged. Inspect
one replay seek/vision switch and one Lab seek/vision switch, confirming each immediately produces a
clean complete view without stale entities or fog.

## Handoff Expectations

Report the final envelope and compact version, sequence semantics, reset reasons, periodic cadence,
float32 byte effect, and tests proving commit-after-send. Tell the Phase 2 sub-agent where the
normalized full baseline lives and how to add patches without moving delta work into the room task.

## Deferred

- Actual section deltas.
- Client keyframe requests or acknowledgements.
- Entity field masks, quantization, compression, countdown/end-tick conversion, event accumulation,
  and transport changes.
