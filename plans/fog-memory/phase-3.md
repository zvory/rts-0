# Phase 3: Expose Remembered Building Intel To The Client

Status: planned

## Goal

Expose server-authoritative remembered building intel to the client only if needed for player
feedback, targeting affordances, or stale silhouettes under fog.

## Scope

- Add a snapshot field for remembered building views, separate from live `entities`.
- Include only recipient-owned memory records that are not currently visible as live entities.
- Mark the records as stale/non-interactive so the client does not treat them as live selectable
  entities.
- Render remembered buildings under fog in a distinct stale-intel style if UI support is required.
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

- Scout an enemy building, leave vision, and confirm the stale marker appears only for that player.
- Confirm stale markers disappear or update when the building is scouted again according to phase 1
  lifecycle rules.
- Confirm stale markers cannot be selected as live enemy entities if that is not intended.
- Confirm spectators/replays do not leak memory from the wrong perspective.

## Handoff

The handoff should document the snapshot shape, compact protocol version, client rendering behavior,
and any deliberate non-interactive restrictions. It should point the next agent to the accepted UI
behavior before pathing work begins.
