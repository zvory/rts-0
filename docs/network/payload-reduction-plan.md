# Network Payload Reduction Plan

This plan reduces the size of the server-to-client game stream before changing transports. It is
written for implementation by a mechanical agent: follow one phase at a time, keep changes scoped,
and do not skip the measurement gates.

## Goal

Make normal gameplay snapshots small enough that TCP head-of-line blocking and browser receive
jitter are much less likely to cause visible stutter.

Current measured baseline from a 1 human + 3 AI quickstart room:

- `start`: about 26 KB
- `snapshot`: about 19.3 KB at 30 Hz
- first measured snapshot contents: 173 entities
- first measured snapshot resource nodes: 168 entities
- same snapshot without resource nodes: about 601 bytes

The first target is therefore not binary encoding or WebTransport. The first target is to stop
resending static resource node data every tick.

## Hard Rules

- Preserve authoritative fog. Do not send enemy units, enemy positions, `targetId`, death events, or
  positional events to a player unless the current fog rules allow it.
- Keep `server/src/protocol.rs`, `client/src/protocol.js`, and `DESIGN.md` in sync whenever a wire
  shape changes.
- Preserve existing `Game::snapshot_for(player) -> Snapshot` and `Game::snapshot_full_for(player) ->
  Snapshot` unless a phase explicitly says to add a new API. Callers outside `game` should continue
  to use the public `Game` seam.
- Do not change simulation behavior while reducing wire payloads. The same commands should produce
  the same authoritative game state.
- Prefer additive protocol changes for one phase before removing old fields. This makes rollback and
  test diagnosis simpler.
- Run the narrow tests listed in each phase before moving on. Run the full live-server suites before
  declaring the whole payload plan complete.

## Payload Budgets

Use these budgets as gates. If a phase misses a budget, stop and investigate before continuing.

- Phase 0 normal early-game snapshot: less than 2 KB p95 for the measured 1 human + 3 AI quickstart
  room.
- Phase 1 normal early-game snapshot: less than 1.5 KB p95 after changed-only resource deltas are
  introduced, if that extra optimization is still needed.
- Phase 3 normal mid-game snapshot: less than 8 KB p95 in an AI self-play or scripted stress room.
- Phase 4 steady-state snapshot delta: less than 4 KB p95 in normal mid-game and less than 10 KB p95
  in full-world dev watch.

These numbers are intentionally practical, not theoretical. Update this document with real measured
numbers after each phase.

## Phase 0: Stop Resending Static Resource Nodes Every Snapshot

Purpose: remove the biggest known waste. Resource positions and kinds are already in the `start`
payload as `map.resources`; snapshots should not resend every static node every tick.

Expected files:

- `DESIGN.md`
- `server/src/protocol.rs`
- `server/src/game/mod.rs`
- `server/src/lobby.rs`
- `server/src/game/services/death.rs`
- `client/src/config.js`
- `client/src/state.js`
- `client/src/renderer.js`
- `client/src/input.js`
- tests near touched behavior

Protocol shape:

- Keep `start.map.resources` as the static resource catalog, and give each resource an `id`.
- Remove resource nodes from network `snapshot.entities`.
- Send visible resource `remaining` through compact `resourceDeltas`.
- When a visible resource is depleted and removed server-side, send a fog-gated `death` event as a
  tombstone so the client can mark its last-known `remaining` as 0.

Implementation steps:

1. Add `id` to the `ResourceNode` / start map resource protocol type.
2. Populate `start.map.resources` from authoritative resource entities with `{ id, kind, x, y }`.
3. Add `resourceDeltas: [{ id, remaining }]` to snapshots.
4. Populate resource deltas only for resources visible to that recipient. Dev full-world watch rooms
   can receive all resource updates.
5. Compact snapshots at the networking boundary by removing resource entities before serialization.
   Keep internal `Game::snapshot_for` behavior available for self-play/replay consumers.
6. Update the client `GameState` constructor to build a `resourceById` map from
   `start.map.resources`.
7. Have the client include known resources in its local entity index so renderer, input hit-testing,
   placement blocking, and gather commands keep working.
8. Apply `resourceDeltas` to the known resource cache.
9. Apply visible resource death events as tombstones by setting last-known `remaining` to 0.
10. Update tests that currently expect resources in snapshots. Prefer changing those tests to
    inspect `start.map.resources`.

Acceptance:

- Network snapshots omit full resource entities.
- `start.map.resources` includes resource ids.
- The player can still click/gather visible starting resources.
- Minimap still shows resource locations.
- Visible depleted resources stop being gather targets.
- `node tests/server_integration.mjs` passes against a running server.
- `node tests/regression.mjs` passes against a running server.
- `cd server && cargo test` passes.

## Phase 1: Optional Payload Metrics Harness

Purpose: make payload size visible in tests and logs if more reduction work is needed. This phase is
optional after Phase 0 because the known waste has already been removed.

Expected files:

- `tests/payload_metrics.mjs` or a helper inside `tests/server_integration.mjs`
- optional: `tests/README.md`

Implementation steps:

1. Add a dependency-free Node script that connects to `RTS_WS`, creates a unique room, adds 3 AI
   players, enables quickstart, starts the match, records raw byte length for each incoming
   WebSocket frame, waits for at least 300 snapshots, and prints JSON metrics.
2. Metrics must include count, min, p50, p95, max for each message type.
3. Snapshot metrics must also include entity count p50/p95/max.
4. Keep the script independent of Puppeteer. Use Node's built-in `WebSocket` like the existing live
   server tests.

Suggested command:

```bash
RTS_WS=ws://127.0.0.1:<port>/ws node tests/payload_metrics.mjs
```

Acceptance:

- The output is machine-readable JSON.
- It does not make local development or CI depend on Puppeteer.
- It is advisory unless a later phase decides to enforce budgets.

## Phase 2: Add Resource Remaining Deltas

Purpose: restore dynamic resource depletion rendering without sending full resource entities.

Expected files:

- `DESIGN.md`
- `server/src/protocol.rs`
- `server/src/game/mod.rs`
- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/renderer.js`
- AI/self-play observation adapters if they read resource remaining from snapshots
- tests for resource depletion and gather behavior

Protocol shape:

Add a compact field to snapshots:

```json
{
  "t": "snapshot",
  "tick": 123,
  "resourceDeltas": [
    { "id": 42, "remaining": 875 }
  ]
}
```

Rules:

- Send a resource delta only when `remaining` changed since the last snapshot sent to that recipient,
  or when the client has just started a match and needs initial dynamic values.
- If per-recipient tracking is too much for this phase, send deltas for all non-full resources every
  snapshot. That is still much smaller than full resource entities.
- A depleted resource must either be represented as `{ id, remaining: 0 }` or by an explicit remove
  event. Prefer `{ id, remaining: 0 }` first.

Detailed steps:

1. Add `resourceDeltas` to `Snapshot` with serde default/skip-empty behavior if practical.
2. Store client-side `remaining` per resource id in `GameState`.
3. Initialize `remaining` from start payload if the start payload includes it. If the start payload
   does not include it, initialize to the known full value by kind.
4. Apply each resource delta before rendering.
5. Renderer resource size should read `remaining` from the static resource model.
6. Gather targeting should ignore depleted resources once `remaining` is 0.
7. Self-play and AI observation adapters should see the same resource id/kind/x/y/remaining facts as
   before, even though they now come from two wire fields.
8. Add tests for:
   - resource starts visible from `start.map.resources`
   - gather increases steel
   - resource remaining drops in the client model after deltas
   - depleted resources are not selected as valid gather targets

Acceptance:

- Resource depletion visuals work.
- Snapshot p95 remains below 1.5 KB in the early-game metric.
- No regression in gather/train integration flow.
- `cd server && cargo test` passes.
- `node tests/server_integration.mjs` and `node tests/regression.mjs` pass against a running server.

## Phase 3: Reduce JSON Entity Field Churn

Purpose: keep full snapshots but make each entity cheaper before implementing true deltas.

Expected files:

- `DESIGN.md`
- `server/src/protocol.rs`
- `server/src/rules/projection.rs`
- `client/src/protocol.js`
- `client/src/state.js`
- focused tests

Work items:

1. Omit fields that can be derived from kind tables or static definitions:
   - `maxHp` if it equals the default max HP for `kind`
   - `owner` for neutral resources, if any still appear in snapshots
   - `state` when it is `"idle"` and clients can default to idle
2. Shorten frequently repeated enum strings only if Phase 1 and 2 are complete and measured:
   - option A: add numeric kind/state codes next to strings for a transition period
   - option B: introduce protocol dictionaries in `start` and send numeric ids in snapshots
3. Keep the first implementation conservative:
   - do not rename every field in one pass
   - do not switch to binary yet
   - do not remove strings until tests prove client and server agree on numeric codes
4. Add protocol mirror tests that fail if a kind/state code diverges between Rust and JS.

Recommended order:

1. Add client defaults for omitted `maxHp` and idle `state`.
2. Server omits those fields when default.
3. Measure.
4. Only then consider numeric kind/state codes.

Acceptance:

- No visible regression in rendering, HUD, selection, combat feedback, production, or construction.
- Snapshot p95 improves in a mid-game/self-play metric.
- Protocol mirror tests exist for any numeric codes introduced.

## Phase 4: Add Snapshot Deltas With Periodic Keyframes

Purpose: avoid sending unchanged unit/building state every tick.

This is the first phase with real complexity. Do not start it until Phases 1 and 2 are complete and
Phase 3 measurements show full snapshots are still too large.

Protocol shape:

- Keep existing `snapshot` as a keyframe message.
- Add a new delta message or a `mode` field:

```json
{
  "t": "snapshotDelta",
  "baseTick": 120,
  "tick": 121,
  "entitiesChanged": [],
  "entitiesRemoved": [],
  "resourceDeltas": [],
  "events": []
}
```

Recommended semantics:

- Server sends a full keyframe every 1 second, or immediately after reconnect/start.
- Server sends deltas on intervening ticks.
- Client applies a delta only if its current snapshot tick equals `baseTick`.
- If a delta is missing or out of order, client ignores deltas until the next keyframe.
- WebSocket is ordered and reliable today, but design the client logic so future datagrams can drop
  deltas safely.

Detailed server steps:

1. Add a per-connection snapshot cache. Do not put this cache inside `Game`; it is transport/output
   state, not authoritative simulation state.
2. Compare the new full projected snapshot against the last sent keyframe/current client image.
3. Build changed entity views by id.
4. Build removed entity ids for entities present in the previous client image but absent now.
5. Include resource deltas and events.
6. Send full keyframes periodically and deltas otherwise.
7. On send queue full or connection trouble, prefer dropping deltas. The next keyframe repairs the
   client.

Detailed client steps:

1. Add `GameState.applySnapshotDelta(delta)`.
2. Keep current entity map by id as the canonical image.
3. For each changed entity, replace the entity in the map.
4. For each removed id, delete from current and previous maps.
5. Preserve interpolation:
   - before applying changes, copy current positions into previous state
   - changed entities interpolate from old position to new position
   - unchanged entities keep stable positions
6. Ignore a delta if `baseTick` does not match the current tick.
7. Ensure selection pruning still happens after deletes.

Tests:

- Unit test or client contract test for applying a delta.
- Server-side test for changed/removed entity calculation.
- Live test that intentionally drops one delta in the client test harness and verifies the next
  keyframe recovers.
- Fog test: enemy disappearing into fog must produce removal for that recipient without leaking the
  hidden position.

Acceptance:

- Normal mid-game p95 delta bytes are below 4 KB.
- Full keyframes still work and are not larger than the Phase 3 full snapshot.
- Client recovers from dropped/ignored deltas within one keyframe interval.
- All live-server suites pass.

## Phase 5: Lower Snapshot Rate or Add Interest Throttling

Purpose: reduce bytes per second after per-message size has been reduced.

Only start after Phase 4. Lowering rate before shrinking messages may hide problems without solving
HOL exposure.

Options:

- Send full simulation ticks at 30 Hz but snapshots at 15 Hz.
- Send nearby combat/own-unit changes every tick and distant visible changes less often.
- Keep pings, commands, notices, game over, and lobby messages reliable and immediate.

Rules:

- Do not change `TICK_HZ` unless the simulation itself needs it.
- Keep client interpolation stable. Update `SNAPSHOT_MS` if snapshot cadence changes.
- Add a server/client config mirror check for snapshot cadence.

Acceptance:

- Player movement remains readable.
- Combat feedback remains acceptable.
- Bytes/sec drops in payload metrics.
- No increase in command-to-visible-feedback latency beyond what measurements justify.

## Phase 6: Consider Binary Encoding

Purpose: reduce overhead after the semantic waste is gone.

Do not start here. Binary encoding should not be used to preserve a wasteful schema.

Options:

- Custom compact binary for snapshots and deltas.
- MessagePack/Postcard-like encoding if it works cleanly in browser JS.
- JSON control messages plus binary snapshot payloads.

Rules:

- Keep command messages JSON unless measurements prove client-to-server traffic matters.
- Add a version byte or magic header to binary payloads.
- Keep a debug mode that can log decoded payloads.
- Add round-trip encode/decode tests for every payload type.
- Keep `DESIGN.md` as the source of truth for the binary format.

Acceptance:

- Binary snapshots are smaller than Phase 4 JSON deltas by a meaningful margin.
- Debuggability remains acceptable.
- Browser client and Rust server have protocol mirror tests.

## Phase 7: Transport Changes After Payload Reduction

Purpose: make WebTransport or another transport useful once messages are small enough.

Do not start transport work until Phase 1 is complete. Prefer waiting until Phase 4 if the goal is
unreliable snapshot delivery.

Likely shape:

- Reliable stream: join, lobby, start, commands, pings/pongs, errors, game over.
- Unreliable datagrams or resettable streams: snapshot deltas.
- Periodic reliable or resettable keyframes: recovery.
- WebSocket remains fallback until WebTransport deployment is proven.

Important constraint:

- QUIC/WebTransport datagrams cannot be treated as arbitrary 20 KB JSON envelopes. If snapshots are
  still large, use streams or reduce payloads further before datagrams.

Acceptance:

- WebSocket fallback still works.
- WebTransport path does not change simulation behavior.
- Dropped snapshot deltas do not break the client.
- Payload metrics can compare WebSocket and WebTransport modes.

## Suggested Final Verification

Run these after the final phase implemented in a branch:

```bash
cd server && cargo test
```

Then start a local server:

```bash
cd server && RTS_ADDR=127.0.0.1:<port> cargo run
```

From the repo root, run:

```bash
RTS_WS=ws://127.0.0.1:<port>/ws node tests/payload_metrics.mjs
RTS_WS=ws://127.0.0.1:<port>/ws node tests/server_integration.mjs
RTS_WS=ws://127.0.0.1:<port>/ws node tests/regression.mjs
RTS_WS=ws://127.0.0.1:<port>/ws node tests/ai_integration.mjs
```

If Puppeteer dependencies are installed:

```bash
cd tests && node client_smoke.mjs
```

Record the final payload metrics in this document or in a follow-up `docs/network/*.md` results file.

## Implementation Notes For Future Agents

- Take exactly one phase per branch unless the phase is documentation-only.
- Start with tests/metrics before changing behavior.
- When a phase changes wire shape, update `DESIGN.md` first or in the same commit.
- If a test expects resource nodes in `snapshot.entities`, update the test to assert the new
  contract instead of preserving the old wasteful behavior.
- If unsure whether something is static or dynamic, treat it as dynamic until there is a test proving
  the client can reconstruct it safely.
- Do not use WebTransport as a shortcut around large snapshots. Reducing payload size is still
  required for both WebSocket and WebTransport.
