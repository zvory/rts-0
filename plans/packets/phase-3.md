# Phase 3 - Delta Snapshot Envelope And Baseline Scaffold

## Phase Status

- [ ] Ready for implementation after Phase 2.6 is merged with MessagePack kept as the accepted
      full-snapshot baseline, and the user explicitly approves moving beyond encoding.

## Objective

Introduce the stateful transport seam for future deltas without depending on a complex delta section
yet. The phase should add a versioned snapshot-frame envelope, per-connection baseline tracking in
the writer path, forced-keyframe rules, and a client-side reconstructor that still hands a normal
semantic snapshot to `GameState.applySnapshot`. A keyframe-only opt-in run through this new path must
behave like the current MessagePack full-snapshot path before later phases shrink any section.

## Background

Phase 2.6 keeps MessagePack as the accepted full-snapshot baseline for delta work. Current live
snapshots are full MessagePack binary frames: the room task builds a fresh per-recipient `Snapshot`,
applies fog projection before send, compacts resources out of the entity list, and enqueues that full
semantic snapshot into a latest-only pending slot. If a client is slow, newer snapshots replace older
unsent snapshots; the writer task later serializes whichever full snapshot it actually takes from
the slot.

That latest-only behavior is only safe for deltas if the server computes deltas from the last frame
that was actually sent to this connection. Do not update a baseline in the room task, and do not
store pending delta chains in `LatestSnapshotSlot`.

## Design Constraints

- Keep default live traffic on the current MessagePack full-snapshot path unless the phase adds an
  explicit opt-in flag for the keyframe-only delta envelope.
- Compute the delta/keyframe frame from an already projected per-recipient `Snapshot`; the encoder
  must never see global hidden simulation state for this recipient.
- Keep `LatestSnapshotSlot` storing full semantic snapshots. Delta encoding belongs in a per-writer
  codec/reconstructor layer at send time.
- Update the server baseline only after the WebSocket send succeeds. If send fails, the connection is
  closing and the baseline can be dropped.
- Keep reliable messages FIFO and prioritized over snapshots. The delta layer must not block the
  room task.
- Keep `GameState.applySnapshot` and renderer/HUD/minimap/input callers receiving semantic snapshots,
  not transport delta frames.
- Force keyframes on `start`, reconnect, unsupported snapshot-frame version, MessagePack/schema version
  change, replay seek, branch promotion/start, lab import/reset/seek/vision change, spectator replay
  vision change, projection-policy change, and a periodic cadence chosen in this phase.
- Treat client recovery requests as advisory and rate-limited; clients remain untrusted.
- Preserve current transient-event behavior unless a later phase explicitly changes it. Events are
  not part of durable baseline state in this phase.

## Proposed Frame Model

The exact compact key names may be adjusted during implementation, but the committed protocol docs
must define one stable shape before code lands. The intended model is:

```json
{ "t": "snapshot", "v": 23, "m": 0, "q": 42, "k": { "...": "compact keyframe body" } }
{ "t": "snapshot", "v": 23, "m": 1, "q": 43, "b": 42, "d": { "...": "delta body" } }
```

- `v` is the new compact snapshot-frame version. Use the actual next version at implementation time.
- `m` is `0` for keyframe and `1` for delta.
- `q` is a per-connection frame sequence.
- `b` is the baseline frame sequence referenced by a delta.
- `k` contains the same compact semantic snapshot body currently represented by `s`, `e`, `r`, `fg`,
  `mb`, `ev`, `pr`, `u`, and `n`, minus the outer `t`/`v` pair if the implementation chooses to nest
  the body.
- `d` is intentionally empty or absent in this phase; later phases define section-specific delta
  contents.

The client decoder should accept the current MessagePack full-snapshot frame and the new
keyframe-only frame. For a keyframe frame, it should decode `k` to the same semantic snapshot object
current code produces. For a delta frame with no supported baseline, it should reject or ignore the
frame without mutating client snapshot state, then request or wait for the next keyframe.

## Work

- Add a server-side snapshot codec/baseline abstraction owned by the writer task:
  - track `frame_seq`, last sent semantic baseline, keyframe cadence, and forced-keyframe reason;
  - encode from the full semantic `Snapshot` taken from `LatestSnapshotSlot`;
  - update baseline only after `sink.send(...)` succeeds;
  - expose metrics for keyframe count, delta count, forced-keyframe reason, and baseline resets.
- Add a client-side snapshot reconstructor below the `Net` dispatch layer:
  - decode current MessagePack snapshots unchanged;
  - decode new keyframe frames into the current semantic snapshot shape;
  - validate frame sequence, baseline sequence, version, mode, and maximum section sizes;
  - drop unsupported or stale delta frames without passing partial state into `GameState`.
- Add a bounded advisory recovery path:
  - either a small `requestSnapshotKeyframe` client message or an equivalent existing-message hook;
  - server-side rate limiting and logging;
  - no gameplay authority or server-state mutation beyond forcing this connection's next snapshot
    frame to be a keyframe.
- Reset or force keyframes at every room/projection seam:
  - match `start` payloads and pending-snapshot clears;
  - replay seek and replay vision changes;
  - branch staging/live promotion;
  - lab import/reset/time-step/seek/vision changes;
  - dev-watch and full-world projection switches.
- Update docs and tests:
  - document the frame envelope, baseline ownership, forced keyframes, and recovery behavior in
    `docs/design/protocol.md`;
  - document new perf/report fields in `docs/perf-tracing.md` if logging/reporting changes;
  - keep full-keyframe recovery behavior documented without restoring compact JSON compatibility
    fallback.

## Expected Touch Points

- `server/src/main.rs`
- `server/src/lobby/connection.rs`
- `server/src/lobby/snapshot_fanout.rs`
- `server/src/lobby/launch.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/net.js`
- `client/src/protocol.js`
- a new client helper such as `client/src/snapshot_reconstructor.js` if that keeps `net.js` small
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- focused Rust protocol and lobby writer tests
- focused client decoder/reconstructor tests

## Handoff Expectations

List the final frame shape, compact version, keyframe cadence, every forced-keyframe trigger, and the
baseline update rule. State whether the new path is default-off, flag-gated, or capability-gated.
Call out any recovery path that was deferred before Phase 4 starts.

## Implementation Checklist

- [ ] Confirm Phase 2.6 kept MessagePack as the full-snapshot baseline, recommends delta work, and
      user approval exists.
- [ ] Add a per-writer snapshot frame codec with baseline reset and keyframe-only support.
- [ ] Keep `LatestSnapshotSlot` storing full semantic snapshots.
- [ ] Add client reconstruction for current MessagePack snapshots and new keyframe frames.
- [ ] Add unsupported/stale delta handling that cannot corrupt `GameState`.
- [ ] Add forced-keyframe/reset handling for start, reconnect, replay, branch, lab, dev-watch, and
      projection-policy seams.
- [ ] Add metrics/logging for keyframe count, keyframe reasons, and baseline resets.
- [ ] Update protocol/perf docs and focused tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- focused client reconstructor tests added in this phase
- focused Rust protocol tests for frame encode/decode and malformed frame rejection
- focused lobby/writer tests proving the baseline updates only after sent frames
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If a local live match is practical, run the keyframe-only opt-in path long enough to receive reports
and confirm gameplay, selection, fog, commands, replay entry, and lab/dev-watch start behavior match
the current compact path.

## Manual Test Focus

Play a normal local match on the default compact path and on the keyframe-only opt-in path. Confirm
that snapshots apply, interpolation remains smooth, commands still acknowledge, fog looks unchanged,
replay seek forces a clean visual reset, lab/dev-watch snapshots render, and unsupported frame
handling produces a clear diagnostic instead of a stuck or corrupted game view.
