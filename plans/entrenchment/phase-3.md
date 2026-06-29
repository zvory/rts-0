# Phase 3 - Dig-In, Occupation, And Slotting

## Phase Status

Status: not started.

## Objective

Implement the core trench lifecycle for units: eligible infantry create trenches after staying
stationary for three seconds, existing trenches can be used immediately, and stopped units may slot
slightly into a trench while preserving collision. This phase should expose occupation state but
should not grant combat bonuses yet.

## Scope

- Add per-unit state needed to track dig-in progress, occupied trench id, and cancellation.
- Start or continue the dig-in timer only for eligible infantry owned by a player with Entrenchment
  research, on untrenched ground, while stationary.
- Do not grant any entrenchment benefits during the 3-second dig-in period.
- Treat firing, weapon facing changes, body facing changes, target changes, and Machine Gunner
  setup/teardown as compatible with staying stationary.
- Cancel or prevent new trench creation on actual commanded movement, path movement, non-slotting
  forced movement, build/gather/deconstruct movement, or any order state that means the unit is no
  longer holding ground.
- Create a permanent neutral trench at the unit's position after the timer completes.
- Let eligible infantry from any player immediately occupy an existing trench when stopped in it,
  even if that player has not researched Entrenchment.
- Implement small slotting movement for stopped eligible infantry near a trench. Slotting must keep
  units standable, preserve spacing, and avoid static blockers.
- Slotting must not count as commanded movement for entrenchment purposes, and a slotting unit must
  still be able to shoot.
- Ensure moving normally through or past a trench does not grant occupation or future combat
  benefits until the unit stops.
- Expose enough projected state for the owner and visible enemies/allies to tell that a visible
  unit is occupying a trench, but do not add final occupied-unit art.
- Expose a simulation helper or predicate that later combat code can use to distinguish active
  trench occupation from digging in, slotting, or merely being near trench terrain. Phase 4 needs
  this to suppress idle pursuit only for actually entrenched units.
- Update `docs/design/server-sim.md` for stationary, creation, occupation, and slotting semantics.

## Expected Touch Points

- `server/crates/sim/src/game/entity/entity.rs`
- `server/crates/sim/src/game/entity/state.rs`
- `server/crates/sim/src/game/systems.rs`
- `server/crates/sim/src/game/services/movement/`
- `server/crates/sim/src/game/services/standability.rs`
- `server/crates/sim/src/game/services/occupancy.rs`
- `server/crates/sim/src/game/services/spatial.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/game/tests/`
- `docs/design/server-sim.md`
- `docs/design/protocol.md` if occupation state adds or changes a wire field

## Verification

- Focused Rust tests for timer start, completion at 90 ticks, cancellation on commanded movement,
  and no cancellation on firing/facing/target changes.
- Tests proving pre-research eligible infantry can use an existing trench but cannot create a new
  one.
- Tests proving Mortar Teams, Ekat, Golems, vehicles, buildings, and excluded support weapons do
  not create or benefit from occupation.
- Slotting tests proving a near-stopped unit moves only to a legal position, does not stack, and
  does not clip terrain/buildings/Tank Traps.
- Projection/fog tests for occupied visible units and hidden occupied units.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if a new
  service is added.
- `node tests/protocol_parity.mjs` if any occupation field changes the wire contract.
- `git diff --check`.

## Manual Test Focus

Research Entrenchment, stop Riflemen or Workers in open ground, and confirm trenches appear after
three seconds and persist after moving away. Move eligible infantry from another player onto an
existing trench and confirm they occupy it without their own research. Check that a moving unit
crossing a trench does not become occupied until it stops.

## Handoff Expectations

Summarize the exact stationary predicate, cancellation cases, slotting radius/shape, occupation
projection field, and combat-facing occupation helper. Call out any edge cases deferred to Phase 4,
especially the idle-pursuit suppression behavior for entrenched units.
