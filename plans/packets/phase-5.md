# Phase 5 - Entity Record Delta Protocol

## Phase Status

- [ ] Ready for implementation after Phase 4 is merged and resource/fog deltas are measured.

## Objective

Implement record-level entity deltas on top of the Phase 3/4 baseline model. This phase should send
full compact entity records for added or changed projected entities and explicit removed ids for
entities that disappeared from this recipient's projected view. It should not attempt field-level
entity patches or auxiliary section deltas.

## Design Constraints

- Diff compact entity records after per-recipient projection. Do not diff raw sim entities.
- Treat one compact entity record as the unit of change. Optional slots such as production progress,
  rally, order plan, abilities, setup state, owner-only debug path, `targetId`, and `weaponFacing`
  travel inside the changed record when that record changes.
- Remove an entity from the client baseline when it is no longer present in the current projected
  snapshot, whether it died, left fog, became unsafe to inspect, or stopped being included for any
  other projection reason.
- Keyframe instead of delta when the changed-record payload plus removed ids is larger than the full
  entity section, crosses safety caps, or references a stale baseline.
- Keep event semantics unchanged. A death event may coexist with an entity removal, but the removal
  is what clears durable entity baseline state.
- Keep resource/fog deltas from Phase 4 enabled only if their fallback behavior is stable; otherwise
  force keyframes or section-full fallbacks before adding entity statefulness.

## Delta Shape

Use the Phase 3 delta frame body (`d`) and define these fields in `docs/design/protocol.md`:

- Changed entities: `d.e = [CompactEntityRecord, ...]`, where each record uses the current compact
  entity slot schema and includes its id in slot 0.
- Removed entities: `d.ex = [id, ...]`.

The client reconstructor should apply removals first, then changed records, then sort or otherwise
emit the reconstructed `entities` array in the deterministic order expected by existing tests and
client code.

## Work

- Add server-side entity diffing:
  - maintain an id-to-compact-record baseline per connection;
  - compare compact records byte/value-wise after projection and compaction;
  - emit changed records for additions and updates;
  - emit removed ids for baseline records absent from the current projected snapshot;
  - keyframe when caps, size comparisons, or baseline mismatch require it.
- Add client-side entity reconstruction:
  - validate compact records using the same bounds as keyframes;
  - reject duplicate changed ids, duplicate removals, unknown compact codes, and oversized sections;
  - remove hidden/dead/out-of-view ids before adding changed records;
  - produce semantic entity objects before `GameState.applySnapshot`.
- Add fog/privacy tests:
  - enemy leaves current fog after being visible;
  - allied unit attacks hidden enemy and does not retain hidden `targetId` or `weaponFacing`;
  - owner-only fields disappear when a projection no longer allows them;
  - lingering death sight/`visionOnly` transitions clear correctly;
  - remembered buildings remain separate from live entity baseline.
- Add measurement:
  - entity changed count, removed count, entity keyframe fallback count;
  - p95/max bytes and over-budget rate against Phase 1/2/4 baselines;
  - reconstruction cost compared with parse/decode/apply.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- Phase 3/4 snapshot codec modules
- `server/crates/sim/src/game/snapshot.rs` tests or fixtures if projection scenarios need coverage
- `server/src/lobby/snapshot_fanout.rs`
- `client/src/protocol.js`
- `client/src/snapshot_reconstructor.js` or the Phase 3 equivalent
- `client/src/state.js` only if a narrow reconstruction boundary needs an explicit hook
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- focused Rust projection/codec tests
- focused JS decoder/reconstructor tests
- `tests/protocol_parity.mjs`

## Implementation Checklist

- [ ] Confirm Phase 4 metrics and fallback behavior are understood.
- [ ] Implement record-level add/update/remove entity deltas after projection.
- [ ] Add client reconstruction and malformed entity delta rejection.
- [ ] Prove removed entities clear baseline state when they leave fog or die.
- [ ] Prove hidden target/tracer and owner-only fields are not retained through baseline reuse.
- [ ] Add metrics for changed records, removed ids, entity fallback keyframes, bytes, and
      reconstruction cost.
- [ ] Update protocol/perf docs and focused tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- focused client reconstructor tests for entity add/update/remove and malformed frames
- focused Rust codec/projection tests for fog exit, owner-only field removal, and hidden target
  retention prevention
- a focused lobby/writer coalescing test for entity deltas against last sent baseline
- `node scripts/check-docs-health.mjs`
- `git diff --check`

When practical, rerun the same packet workloads used in Phase 4 and record p95 payload bytes,
over-budget rate, changed/removed entity counts, keyframe fallback rate, and reconstruction p95.

## Manual Test Focus

Play with fog enabled and move units in and out of enemy vision. Confirm enemy units appear, update,
disappear under fog, die cleanly, and do not leave stale selectable/targetable ghosts. Also test
owner-only command-card data, production/rally/order-plan visibility, allied combat against hidden
targets, spectator/replay views, and full-world lab/dev-watch snapshots.

## Handoff Expectations

State whether record-level entity deltas produced enough savings to justify auxiliary section work.
List privacy/fog scenarios covered by tests and any remaining scenario that needs manual inspection.
Tell Phase 6 which sections still dominate payloads and whether any entity optional fields should
stay record-level rather than becoming field-level patches.
