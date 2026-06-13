# Phase 2 - Viewport Army Value Overlay

Status: Done.

## Objective

Display each player's visible on-screen army value in steel and oil while watching a replay fight.
This phase should be client-only and derived from the current replay snapshot, selected replay
vision, current camera viewport, and mirrored unit costs.

## Definition

Army value in this phase means:

- units only, not buildings, resources, smoke clouds, remembered buildings, or projectiles
- current visible replay snapshot only
- current camera viewport only
- full unit cost, not HP-adjusted value
- grouped by entity `owner`
- based on `STATS[kind].cost` from the client balance mirror

`shotReveal` temporary attack reveals are excluded because they are short-lived muzzle-position
projections rather than current on-screen army presence. `visionOnly` lingering death-vision units
are included while the server projects them because this overlay describes the replay viewer's
currently visible fight state, not whether the entity is actionable command vision.

## Scope

- Add an army value calculator that accepts entities, camera bounds, player metadata, and cost
  lookup data.
- Render a compact per-player readout in the Phase 1 overlay shell.
- Update the readout every frame or only when the current snapshot/camera signature changes; prefer
  a simple implementation first unless performance measurements show a problem.
- Handle empty viewports and players with zero visible on-screen army value.
- Use player colors and names from `state.players`.
- Avoid protocol/server changes.
- Keep the overlay readable on desktop and mobile without overlapping the replay speed/timeline
  controls, minimap, or top resource HUD.

## Expected Touch Points

- Phase 1 replay analysis overlay module
- `client/src/match.js` frame update wiring if the overlay updates per frame
- `client/src/config.js` read-only import of `STATS`
- `client/src/protocol.js` read-only import of `isUnit`
- `client/styles.css`
- targeted client test files if a calculator unit test location exists or is added

## Verification

- Unit-test the pure army-value calculator:
  - excludes buildings and resources
  - excludes off-screen units
  - groups values by owner
  - handles missing/unknown costs as zero or an explicit ignored state
  - respects the final `visionOnly` / `shotReveal` decision
- Run:

```bash
node scripts/check-client-architecture.mjs
```

- Run a targeted browser smoke test if an existing smoke harness can open replay/dev-watch and
  inspect overlay text. Otherwise document the manual verification and leave automated replay UI
  smoke as a Phase 5 hardening item.

## Manual Testing Focus

Open a replay with a visible fight, pan and zoom around the battlefield, and confirm values change
as units enter and leave the viewport. Switch replay vision between all players and a single player
and confirm the overlay follows the server-projected visible entities. Seek backward and forward
and confirm the overlay updates after the replay rebuilds.

## Handoff Expectations

The handoff must state the exact army-value definition, the treatment of `visionOnly` and
`shotReveal`, the test coverage added, and any layout compromises. The next agent should proceed
to Phase 3 only after the client-only overlay is stable through replay seeking.

## Implementation Handoff

- Army value is computed client-only from the current replay snapshot entities, the current camera
  world bounds, `state.players`, and `STATS[kind].cost`. It counts units only, groups by owner, uses
  full steel/oil cost, treats missing costs as zero, and keeps zero-value rows for known players.
- Viewport inclusion uses the unit render radius from `STATS[kind].size` so units partially inside
  the camera viewport are counted. Buildings, resources, neutral entities, off-screen units, and
  `shotReveal` attack pings are ignored.
- `visionOnly` lingering death-vision units are included while visible in the replay-projected
  snapshot. This matches visible fight-state analysis; Phase 3+ should call out any later global
  analysis fields that instead represent actionable or fully authoritative hidden state.
- The readout is rendered only inside the Army Value tab in the Phase 1 overlay shell. Layout is a
  compact top-right panel with player swatches, names, and tabular steel/oil columns; long names
  ellipsize to avoid overlapping replay controls, the minimap, or the resource HUD.
- Added calculator and overlay DOM coverage in `tests/client_contracts.mjs`. Verified with
  `node tests/client_contracts.mjs` and `node scripts/check-client-architecture.mjs`.
- Automated replay UI smoke was not added in this phase. Manual verification should open a replay,
  pan/zoom over a fight, switch replay vision, and seek to confirm values rebuild with the snapshot
  and viewport before Phase 3 starts.

## Player-Facing Outcome

Replay viewers can judge the visible equipment value in a fight before or during engagement,
without clicking units or inferring composition manually.
