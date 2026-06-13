# Phase 4 - Shared Vision, Projection, and Event Delivery

Status: planned.

## Goal

Make allied line of sight and events work without leaking hidden enemy information. Allies should
share current vision, explored history should naturally accumulate on the client, and support-fire
markers such as mortar fire should be visible to teammates.

## Scope

- Recompute authoritative current fog by team, or stamp each entity's sight into every allied
  player's grid.
- Keep neutral resource nodes from granting vision.
- Preserve smoke blocking and lingering death sight semantics under team vision.
- Project full allied entity details:
  - hp/state/facing/setup state
  - production kind/progress/queue length
  - research progress/queue length
  - build progress
  - worker latched node
  - safe combat target ids and weapon facing
- Keep local-player-only details private where appropriate:
  - resources
  - supply
  - upgrades
  - rally and order plans unless deliberately made ally-visible
  - command authority
- Deliver team-safe events:
  - allies receive events for allied attacks, deaths, construction, mortar launches, mortar impacts,
    artillery target markers, artillery impacts, smoke launches, and under-attack notices as
    appropriate
  - enemies receive events only when their team can currently see the relevant origin or target
  - hidden enemy positions and `targetId` tracers must not leak to any team member
- Ensure `visibleTiles` sent to each player reflects team current vision.
- Ensure remembered buildings are based on team-visible observations without duplicating or leaking
  stale hidden positions incorrectly.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/hardening.md`
- `server/crates/sim/src/game/fog.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/sim/src/game/building_memory.rs`
- `server/crates/sim/src/game/mortar.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/game/smoke.rs`
- `server/crates/sim/src/game/services/combat/events.rs`
- `server/crates/sim/src/game/services/death.rs`
- `tests/team_integration.mjs`
- `tests/regression.mjs`
- `tests/tri_state/scenarios/` if a compact hidden-target leak scenario is useful

## Verification

```bash
cd server && cargo test fog --workspace
cd server && cargo test projection --workspace
cd server && cargo test team --workspace
node tests/team_integration.mjs
node tests/regression.mjs
```

Required automated scenarios:

- Ally scout reveals an enemy to a teammate's snapshot.
- Enemy outside all allied sight is absent from every teammate snapshot.
- Allied production building exposes full read-only details.
- Player with no own entities but living allies still receives team vision.
- Mortar launch/target markers are visible to the firing player's allies.
- Artillery point-fire markers and launch/impact events follow explicit team visibility rules.
- Hidden enemy `targetId`, death positions, attack reveals, and remembered buildings do not leak
  through ally sharing.
- Shared `visibleTiles` updates cause client explored history to accumulate from allied vision in a
  headless or smoke-testable path.

## Acceptance Criteria

- Server-authoritative shared current vision works for every teammate.
- Allied full-detail snapshots work without granting command authority.
- Support-fire markers are visible to allies.
- Hidden enemy data remains hidden from the whole opposing team.

## Manual Testing Focus

Use one scripted browser/dev scenario, if available, to visually confirm allied mortar/artillery
markers and fog dimming. Manual multi-tab setup should not be required.

## Handoff Requirements

The phase handoff must identify the projection privacy rules, list event types audited, and note any
event intentionally kept owner-only.
