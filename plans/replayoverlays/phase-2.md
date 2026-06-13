# Phase 2 - Viewport Army Value Overlay

Status: Not implemented.

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

`visionOnly` shot/death-reveal entities need an explicit product decision during implementation.
The recommended default is to exclude `shotReveal` temporary attack reveals and include or exclude
`visionOnly` lingering death-vision units based on whether the overlay is meant to describe visible
fight state or actionable current vision. Document the final choice in this phase file and handoff.

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

## Player-Facing Outcome

Replay viewers can judge the visible equipment value in a fight before or during engagement,
without clicking units or inferring composition manually.

