# Phase 3: Expose Remembered Building Intel To The Client

Status: implemented with phase 2 after artillery targeting semantics were clarified

## Goal

Expose server-authoritative remembered building intel to the client only if needed for player
feedback, targeting affordances, or normal remembered building rendering under fog.

## Scope

- Add a snapshot field for remembered building views, separate from live `entities`.
- Include only recipient-owned memory records that are not currently visible as live entities.
- Mark the records as stale/non-interactive so the client does not treat them as live selectable
  entities.
- Render remembered buildings with the normal building renderer below the fog overlay if UI support
  is required.
- Keep target commands validated server-side; client rendering is advisory only.

## Important Design Choices

- Do not overload `entities` with hidden stale records unless the rendering stack already has a
  safe `visionOnly`/non-interactive path that cannot leak commandability.
- The record should include enough shape information to render a footprint, but not live hidden hp
  or state beyond last seen.
- Compact protocol slot changes must be versioned and backwards-compatible with existing optional
  slots.
- Replay vision should use the selected player's remembered records, not spectator omniscience,
  unless explicitly in full-world/dev mode.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/src/protocol.rs` if adapter changes are needed
- `client/src/protocol.js`
- Relevant client state/render modules
- `docs/design/protocol.md`
- Possibly `docs/design/client-ui.md`

## Verification

- `cd server && cargo test snapshot`
- Protocol serialization/deserialization tests if available
- `node tests/regression.mjs` with a running server if protocol behavior changes
- `tests/run-all.sh --no-rust` when the client render path changes

## Manual Testing Focus

- Scout an enemy building, leave vision, and confirm the remembered building remains rendered below
  fog only for that player.
- Confirm stale markers disappear or update when the building is scouted again according to phase 1
  lifecycle rules.
- Confirm stale markers cannot be selected as live enemy entities if that is not intended.
- Confirm spectators/replays do not leak memory from the wrong perspective.

## Handoff

Snapshot shape: `Snapshot.rememberedBuildings` / compact `mb` contains
`{ id, owner, kind, x, y, footprint, observedTick }` for recipient-only remembered enemy buildings
that are not currently visible as live entities. Compact snapshot version is 17.

Client rendering behavior: remembered buildings are stored outside the live entity index and drawn
through the normal building renderer beneath the fog overlay. They are not selection or
attack-target entities, and they do not get live-entity overlays like HP bars or selection rings.

Restriction: records intentionally omit hidden live HP, current build progress, and destruction
state. Hidden destruction remains stale until the remembered footprint is scouted again.
