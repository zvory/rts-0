# Phase 2 - First Useful Live Deltas

## Phase Status

- [ ] Ready after Phase 1 is merged and its keyframe-only path is verified.

## Objective

Make deltas useful for normal live active players without generalizing every viewing mode or entity
field. Compare each newest full fog-filtered snapshot with that writer's last successfully sent
baseline, send sparse absolute entity and fog changes, and reconstruct the exact complete semantic
snapshot before existing client state sees it.

Spectator, replay, Lab, branch, and dev-watch connections remain on Phase 1 keyframes during this
phase.

## Design Constraints

- Keep full snapshots in the latest-only pending slot and all diffing in the writer-owned encoder.
- Diff only recipient-projected state. Skipped ticks are irrelevant: the newest snapshot compares
  directly with the last one sent.
- Carry authoritative absolute values, never arithmetic differences.
- Commit a delta baseline and sequence only after the WebSocket send succeeds.
- Force a keyframe when no valid baseline exists, Phase 1 requires one, a patch crosses its bounds,
  reconstruction would be ambiguous, or the encoded delta is not smaller than the keyframe.
- Do not add general entity masks, quantization, compression, event acknowledgement, or generalized
  collection machinery in this phase.

## Delta Shape

Fit the following into the Phase 1 delta body and bump the current compact snapshot version:

```text
d.s  = [tick, steel, oil, supplyUsed, supplyCap]
d.et = [[id, x, y, facingOrNull, weaponFacingOrNull], ...]
d.e  = [CompactEntityRecord, ...]
d.ex = [id, ...]
d.fg = [[startIndex, runLength, newValue], ...]
d.r  = [[id, remaining], ...]
ev   = [EventRecord, ...]
n    = CompactNetStatus
```

Entity rules:

- New entity: send a complete record in `d.e`.
- Only `x`, `y`, `facing`, or `weaponFacing` changed: send an absolute hot transform in `d.et`;
  `null` explicitly clears an optional facing.
- Any other projected field changed: send the complete current entity record in `d.e`.
- Entity absent from the current projected snapshot: send its id in `d.ex`, whether it died, left
  fog, or lost projection eligibility.
- Ids in the three entity patch sets are disjoint. A transform may reference only an entity already
  present in the baseline.
- The client applies removals, full upserts, and transforms in that order, then emits deterministic
  id ordering.

Fog rules:

- Compare equal-length row-major grids and group changed tiles into sorted, non-overlapping absolute
  `[startIndex, runLength, newValue]` runs.
- Validate bounds and values on the client. A grid length change, overlap, malformed run, or patch
  larger than the full fog representation forces or waits for a keyframe without partial mutation.

## Section Policy

| Section | Phase 2 policy |
| --- | --- |
| Scalars and `netStatus` | Send in full every frame. |
| Entities | Complete add/replace, hot transform, explicit remove. |
| `visibleTiles` | Absolute changed runs. |
| `resourceDeltas` | Send the outgoing frame's coalesced newest absolute value per id; never baseline it as a complete section. |
| `events` | Send the latest outgoing snapshot's full transient list; never put it in the durable baseline. Preserve current best-effort replacement semantics. |
| `smokes`, `abilityObjects`, `trenches`, `rememberedBuildings`, `upgrades` | Send a full replacement only when changed; omission means unchanged and an explicit empty section clears. |
| `worldCombatPosition` | Replace only when changed; explicit `null` clears. |
| `playerResources` | A non-empty observer section forces a keyframe because this phase is active-player only. |

## Work

- Add the active-live-player delta encoder beside the compact keyframe encoder.
- Teach the writer to compare the taken full snapshot with its last successfully sent normalized
  baseline and encode both delta and keyframe candidates for the size choice.
- Preserve pending resource-update coalescing. Ensure replacement retains the newest absolute value
  per node rather than an arbitrary overwritten value.
- Extend the client reconstructor to validate and atomically apply entity, fog, scalar, stable-section,
  resource, event, and net-status policies before returning a normal semantic snapshot.
- Keep transport concerns out of `GameState`, renderer, HUD, minimap, input, and prediction.
- Add bounded diagnostics for keyframe/delta counts, fallback reasons, entity full/transform/remove
  counts, fog-run count, encoded bytes, server diff time, and client reconstruction time.
- Update the protocol source of truth and Rust/browser compact mirrors together.

## Required Correctness Cases

- Visible movement produces an absolute transform patch.
- Spawn produces a complete add; death and fog exit produce explicit removal with no ghost.
- HP, state, order, ability, production, or optional-detail changes produce a complete replacement.
- Hidden `targetId`/`weaponFacing` and owner-only fields do not survive after projection stops
  permitting them.
- Fog changes across several skipped ticks reconstruct exactly to the newest grid.
- Malformed, overlapping, out-of-bounds, duplicate-id, or stale-base patches do not mutate state.
- Pending ticks 101 through 104 replaced by 105 produce a delta from the last sent tick 100.
- Coalesced resource updates retain the newest value for each id.
- Frame-local events and resource updates do not repeat on the next delta unless newly supplied.
- Property-style baseline/current projected pairs reconstruct exactly and preserve deterministic
  entity ordering.

## Expected Touch Points

- `server/src/connection_writer.rs`
- `server/src/lobby/connection.rs`
- Phase 1 stream encoder/baseline module
- `server/crates/protocol/src/lib.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/protocol/src/contract_metadata.rs`
- `client/src/protocol_snapshot.js`
- Phase 1 client reconstructor
- `client/src/net.js` if reconstruction metrics are recorded there
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/protocol_parity.mjs`
- focused Rust connection/codec tests and JS reconstructor tests

## Implementation Checklist

- [ ] Define and document the active-player delta body and compact version.
- [ ] Add complete entity add/replace, hot transform, and explicit remove patches.
- [ ] Add absolute changed fog runs.
- [ ] Implement the section policy, including explicit clears and transient exclusions.
- [ ] Compare delta/keyframe encoded size and use the smaller safe frame.
- [ ] Reconstruct atomically into the existing semantic snapshot shape.
- [ ] Add coalescing, fog/privacy, malformed-input, and property-style tests.
- [ ] Add bounded delta/fallback/processing diagnostics.
- [ ] Record a fixed full-keyframe versus delta workload comparison when practical.
- [ ] Mark this phase done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol`
- Focused `rts-server` connection-writer and coalescing tests.
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- Focused client reconstructor tests.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Play one normal fog-enabled match. Check movement, combat, production, entities entering/leaving
vision, resource harvesting/depletion, selection cleanup, fog edges, and reconnect/keyframe recovery.
Confirm spectator, replay, and Lab views still work through their intentionally retained keyframe
path.

## Handoff Expectations

Report the final compact keys and caps, keyframe fallback rules, exact reconstruction/fog tests,
payload P95 and over-budget comparison, keyframe ratio, server diff P95, and client reconstruction
P95. Name the remaining dominant sections for the Phase 3 sub-agent and call out any stale-state or
privacy scenario not yet covered.

## Deferred

- Deltas for spectator, replay, Lab, branch, and dev-watch modes.
- Record deltas for auxiliary sections beyond the simple replacement policy.
- Countdown-to-end-tick conversion and general entity field masks.
- Guaranteed transient-event accumulation.
- Quantization, compression, or transport changes.
