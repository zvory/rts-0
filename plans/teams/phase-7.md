# Phase 7 - Projection, Memory, and Event Privacy

Status: planned.

## Goal

Make allied entity inspection details and team-visible events work without leaking hidden enemy
information. This phase owns the privacy boundary above shared current fog.

## Scope

- Project full allied entity details:
  - hp/state/facing/setup state
  - production kind/progress/queue length
  - research progress/queue length
  - build progress
  - worker latched node
  - safe combat target ids and weapon facing
- Keep local-player-only details private:
  - resources
  - supply
  - upgrades
  - rally and order plans unless deliberately made ally-visible
  - command authority
  - debug path overlays unless deliberately made ally-visible
- Ensure remembered buildings are based on team-visible observations without duplicating or leaking
  stale hidden positions incorrectly.
- Deliver team-safe events:
  - allies receive events for allied attacks, deaths, construction, mortar launches, mortar impacts,
    artillery target markers, artillery impacts, smoke launches, and under-attack notices as
    appropriate.
  - enemies receive events only when their team can currently see the relevant origin or target.
  - hidden enemy positions and `targetId` tracers must not leak to any team member.
- Audit resource deltas, smoke cloud views, shot reveals, target tracers, mortar/artillery markers,
  death positions, and attack reveals against team visibility.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/hardening.md`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/sim/src/game/building_memory.rs`
- `server/crates/sim/src/game/mortar.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/game/smoke.rs`
- `server/crates/sim/src/game/services/combat/events.rs`
- `server/crates/sim/src/game/services/death.rs`
- `server/src/lobby/snapshots.rs`
- `tests/team_integration.mjs`
- `tests/regression.mjs`
- `tests/tri_state/scenarios/` if a compact hidden-target leak scenario is useful

## Verification

```bash
cd server && cargo test projection --workspace
cd server && cargo test fog --workspace
cd server && cargo test team --workspace
node tests/team_integration.mjs
node tests/regression.mjs
```

Required automated scenarios:

- Allied production building exposes full read-only details.
- Allied resources/supply/upgrades/rally/order plans remain private.
- Mortar launch/target markers are visible to the firing player's allies.
- Artillery point-fire markers and launch/impact events follow explicit team visibility rules.
- Hidden enemy `targetId`, death positions, attack reveals, and remembered buildings do not leak
  through ally sharing.
- Enemy event delivery uses team current vision, not just the individual viewer's own units.
- Resource deltas and smoke views follow the documented team-visibility rules.

## Acceptance Criteria

- Allied full-detail snapshots work without granting command authority.
- Support-fire markers are visible to allies.
- Hidden enemy data remains hidden from the whole opposing team.
- Every event type touched by combat, construction, death, artillery, mortar, and smoke has an
  explicit owner/team/enemy visibility rule.

## Manual Testing Focus

Use one scripted browser/dev scenario, if available, to visually confirm allied mortar/artillery
markers and fog dimming. Manual multi-tab setup should not be required.

## Handoff Requirements

The phase handoff must identify the projection privacy rules, list event types audited, and note any
event or entity field intentionally kept owner-only.
