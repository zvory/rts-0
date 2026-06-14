# Phase 3 - Scout Car Harassment Rewrite

Status: Planned.

## Objective

Rewrite Scout Car harassment route selection to use atlas-backed route queries. Scout Cars should
choose a reachable, vehicle-appropriate route toward enemy resource-line pressure rather than a
single geometric flank waypoint derived from start and resource vectors.

## Scope

- Replace or wrap the existing Scout Car harassment route calculation so it requests route options
  from the Phase 2 query layer.
- Preserve current harassment product requirements:
  - reserve a small Scout Car group separately from frontal-wave units
  - target public enemy starts and public or visible resource-line information without cheating
  - use ordinary `Move` commands through `AiActionContext` and `ai_core::actions`
  - react to visible combat units with existing fog-respecting evasion behavior
  - avoid implementing smoke, worker focus, hidden building ignores, regroup micro, or full
    split-attack control unless a later plan explicitly adds them
- Choose route options with explicit scoring. Prefer routes that are reachable, meet Scout Car
  clearance needs, approach the resource line from a non-front angle when possible, and avoid simply
  duplicating the main frontal route.
- Queue a bounded number of waypoints, likely 2-4, so the command stream represents a route intent
  without flooding queued moves.
- Keep safe fallback behavior. If the atlas is unavailable, every route is rejected, or the route is
  effectively complete, skip harassment for that think or use the previous safe direct behavior
  rather than panicking or repeatedly issuing poor orders.
- Update tests so they assert atlas-backed route properties: component match, movement class,
  clearance threshold, non-front route preference when available, and fallback when unavailable.
- Update trace details if needed so route selection can be diagnosed from compact AI traces.
- Update `docs/design/ai.md` to describe atlas-backed Scout Car harassment.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/harassment.rs`
- `server/crates/ai/src/ai_core/decision/mod.rs`
- Route query API from Phase 2
- `server/crates/ai/src/ai_core/decision/trace.rs` if trace fields change
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/selfplay/` only if first-harassment-command metadata needs updates
- `docs/design/ai.md`

## Verification

Run focused AI decision tests:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai scout_car_harassment
cargo test --manifest-path server/Cargo.toml -p rts-ai routing
```

If command queue behavior changes in shared action helpers, also run:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai ai_core::actions
```

## Manual Testing Focus

Run or spectate a two-AI Default-map game with the current AI profile. Confirm Scout Cars separate
from the main army, move toward resource-line pressure through a route that appears reachable and
credible, and do not starve the frontal-wave plan.

## Handoff Expectations

The handoff must name the selected route behavior, list the focused tests that protect it, and
state every fallback case where harassment skips or uses older direct behavior. It must include
factual patch-note bullets describing the player-facing harassment change.

## Player-Facing Outcome

AI Scout Cars should pressure resource lines through more credible reachable routes. They should no
longer depend on a brittle geometric flank point that can send them into tactically poor terrain.
