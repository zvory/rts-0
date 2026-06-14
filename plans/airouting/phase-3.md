# Phase 3 - Scout Car Harassment Integration

Status: Planned.

## Objective

Use the static route evaluator for Scout Car harassment so the manager issues a short corridor route
instead of one geometric flank waypoint plus a final target. The known right-side top-right versus
bottom-right spawn case should no longer send both AIs through the same one-tile right-edge choke.

## Scope

- Replace or wrap `scout_car_harassment_route` so it asks the route evaluator for a selected
  harassment corridor.
- Preserve the existing high-level harassment policy knobs where they still make sense, including
  group size, reissue cadence, back offset, side offset, and visible threat radius.
- Emit route waypoints through the existing `actions::move_units` and
  `actions::move_units_with_queue` helpers. Do not bypass `AiActionContext` or create custom command
  emission.
- Queue a bounded number of waypoints, likely 2-4, so Scout Cars have a corridor intent without
  flooding the command queue.
- Keep the old direct/fallback behavior available when terrain data is missing, no route candidate
  is acceptable, or the route already appears complete.
- Update harassment tests so they assert corridor properties rather than only "outer flank farther
  from the direct line." Add a specific test for the right-side pairing that proves the chosen first
  waypoint is not the far-right one-tile choke around tile `(126, 63)`.
- Update manager trace output if needed so harassment route selection can be diagnosed without
  reading command coordinates by hand.
- Update `docs/design/ai.md` to describe terrain-aware Scout Car harassment.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/harassment.rs`
- Route evaluator module from Phase 2
- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/trace.rs` if trace fields change
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/selfplay/replay.rs` only if first-harassment-command detection needs route
  metadata changes
- `docs/design/ai.md`

## Verification

Run focused AI decision tests:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai scout_car_harassment
cargo test --manifest-path server/Cargo.toml -p rts-ai routing
```

If command queue behavior changes in a way that touches shared action helpers, also run:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai ai_core::actions
```

## Manual Testing Focus

Run or spectate a two-AI Default-map game with top-right and bottom-right starts. Confirm Scout Car
harassment does not intentionally drive both groups into the far-right middle choke and that ordinary
frontal waves still move toward enemy bases.

## Handoff Expectations

The handoff must name the selected route behavior for the right-side spawn case, list the focused
tests that protect it, and note any fallback cases where harassment can still use the old geometric
targeting. It should also tell Phase 4 what trace or state can be used to detect route failure.

## Player-Facing Outcome

AI Scout Cars should take more credible flanking routes and avoid the obvious right-edge choke loop
in the known top-right versus bottom-right spawn configuration.
