# Phase 2: Artillery Uses Remembered Buildings

Status: planned

## Goal

Make artillery's fogged building targeting depend on the firing player's remembered building intel
instead of omniscient live building positions.

## Scope

- Find the current artillery acquisition/point-fire prioritization path and isolate the rule that
  chooses fogged building targets.
- For currently visible targets, keep using live entity state and current fog checks.
- For fogged enemy buildings, consult building memory for the firing player:
  - Never-seen buildings are ineligible.
  - Seen-then-fogged buildings are eligible at their remembered position/footprint.
  - Stale memory may cause wasted shots if the building moved from construction cancellation,
    died, or changed state out of sight.
- Avoid using remembered records to reveal target ids, hidden live hp, or hidden destruction.
- Keep artillery impact/damage authoritative against live entities at impact time.

## Important Design Choices

- Artillery can aim at stale remembered positions, but damage resolution must remain live and
  authoritative.
- If a remembered building id no longer exists, artillery should still be able to shoot the last
  known position if the order logic supports point targets; otherwise stale ids should safely fail.
- Target priority should prefer visible live threats over stale fog memory unless the current
  artillery design says otherwise.
- Any event generated for the firing player should reveal only the shot marker/impact allowed by
  current artillery event rules, not hidden building state.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/game/services/commands.rs`
- The phase 1 memory module
- Focused tests in combat/artillery command areas

## Verification

- `cd server && cargo test artillery`
- `cd server && cargo test combat`
- `cd server && cargo test commands`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Testing Focus

- Artillery should not prioritize or fire at a never-scouted fogged enemy building.
- After scouting an enemy building and losing vision, artillery may target the remembered location.
- If the enemy destroys or changes that building out of sight, artillery may waste fire on stale
  intel but must not reveal the live outcome until vision/impact rules allow it.
- Current visible artillery targeting should feel unchanged.

## Handoff

The handoff should identify exactly which artillery flows use memory, which still require live
visibility, and any player-facing patch-note implications. It should tell the next agent whether UI
support for stale building silhouettes is needed before moving on.
