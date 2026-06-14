# Phase 1 - Terrain Observation

Status: Planned.

## Objective

Carry public static terrain from the match start payload into AI observations and add small AI-side
map helpers. This phase should not change live AI decisions yet; it only makes terrain available in
the same fog-safe observation layer that already carries public starts and public resource
positions.

## Scope

- Extend `AiMapSummary` or add a sibling AI map type so it can expose row-major terrain codes from
  `StartPayload.map.terrain`.
- Add AI-side helper methods for tile indexing, bounds checks, tile passability, world-to-tile
  conversion, and tile-center conversion where needed by later routing phases.
- Preserve existing observation constructors for live and self-play paths.
- Update unit-test fixtures that construct `AiObservation` by hand so they use a small passable map
  by default.
- Add focused tests proving terrain is copied from `StartPayload` into `AiObservation` and that
  helper methods handle bounds and blocked tiles without panicking.
- Update `docs/design/ai.md` to say AI observes public static terrain and may use it for route
  planning, while still not reading private simulation state.

## Expected Touch Points

- `server/crates/ai/src/ai_core/observation.rs`
- `server/crates/ai/src/ai_core/decision/geometry.rs` or a new AI map helper module
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/ai_core/facts.rs` tests if observation fixture helpers live there
- `docs/design/ai.md`

## Verification

Run focused AI crate tests that cover observation and decision fixtures:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai observation
cargo test --manifest-path server/Cargo.toml -p rts-ai ai_core::decision
```

If the exact test filters miss new test names, run the smallest `rts-ai` test filter that includes
the added observation/map-helper coverage.

## Manual Testing Focus

No gameplay manual test is required for this phase because decisions should not change. If a manual
sanity check is desired, start a local AI match and confirm the lobby can still add AIs and begin a
match without controller startup errors.

## Handoff Expectations

The handoff must name the AI map/terrain helper API added in this phase, call out whether terrain is
copied or borrowed from the start payload, and tell Phase 2 which helper to use for route candidate
scoring.

## Player-Facing Outcome

No intended player-facing change. This phase only gives later AI route planning code safe access to
public static terrain.
