# Phase 1 - State Ownership Inventory

Status: Done.

## Scope

Create the first explicit, reviewable state ownership registry for the current `Game` aggregate
before moving code into `GameState` / `DerivedState` structs or introducing durable checkpoint DTOs.
This phase is docs/registry work only and must preserve behavior.

The registry must classify every field currently stored on `server/crates/sim/src/game/mod.rs`
`Game` into exactly one checkpoint policy category:

- `authoritative/serialized`: durable state that can change future simulation results, command
  validity, fog/projection, replay output, scoring, entity ids, or checkpoint restore behavior;
- `derived/rebuildable`: cache, index, or search state that can be cleared at a tick boundary and
  rebuilt from authoritative state without changing semantic results or fog-filtered snapshots;
- `transient`: intentionally dropped runtime state that cannot affect future authoritative behavior;
- `compatibility metadata`: metadata retained for replay/API/setup compatibility that may not mutate
  tick results directly, but must still have an explicit checkpoint policy.

The inventory should remove ambiguity around at least these hard cases:

- `pending` commands and whether checkpoint boundaries may contain unapplied commands;
- `command_log` and whether command history is part of checkpointed state or replay metadata;
- `active_construction_sites`, including its role in construction progress projection during the
  tick where work occurred;
- `spatial` and any other spatial/occupancy read model that is rebuilt from map/entities;
- `pathing`, separating reusable cache/search bookkeeping from authoritative chosen unit paths,
  movement phases, waypoints, path goals, and throttling stored on entities;
- `seed`, `starting_loadouts`, `map_metadata`, and legacy `starting_loadout` compatibility/setup
  metadata;
- `lab_god_mode_players` and the mirrored invulnerability flags on lab-owned units/buildings;
- `rng`, including current generator state versus original match seed;
- fog, building memory, trench memory, lingering sight, firing reveal, smoke, trench, ability
  runtime, mortar shell, and artillery shell stores.

Do not move Rust fields, add checkpoint serialization, change replay behavior, change lab behavior,
or edit client/server implementation code in this phase. If a field's category is uncertain, keep
the uncertainty in the registry as an explicit blocker or follow-up rather than guessing silently.

## Expected Touch Points

- `docs/design/server-sim.md`: add the authoritative state ownership registry, close to the `Game`
  API/services/derived-state discussion.
- `docs/context/server-sim.md`: update only if the new registry adds or shifts a section that future
  agents should read from the capsule.
- `plans/game-state/plan.md`: refresh Phase 1 status/summary text if the implementation changes the
  plan's phase index.
- `plans/game-state/phase-1.md`: mark the phase complete only in the implementation commit that
  lands the registry.

Implementation Rust/JS files, including `server/crates/sim/src/game/mod.rs`, are read-only evidence
for this phase.

## Verification

- Confirm the registry has one row or bullet for every current `Game` field:
  `map`, `entities`, `fog`, `building_memory`, `players`, `pending`, `command_log`, `tick`,
  `spatial`, `pathing`, `lingering_sight`, `firing_reveals`, `smokes`, `trenches`,
  `ability_runtime`, `mortar_shells`, `artillery_shells`, `seed`, `starting_loadouts`,
  `map_metadata`, `active_construction_sites`, `lab_god_mode_players`, `starting_loadout`, and
  `rng`.
- For each field, record the chosen category, checkpoint policy, rebuild/drop rule if applicable,
  and at least one evidence note from current code or design docs.
- Cross-check the registry against Phase 0.5's derived-state assumptions. Any mismatch must be
  explained in the registry or captured as a follow-up before checkpoint DTO work starts.
- Run a docs-only sanity check such as:

```bash
git diff --check -- docs/design/server-sim.md docs/context/server-sim.md plans/game-state/plan.md plans/game-state/phase-1.md
```

No Rust or Node test is required unless the registry phase accidentally changes implementation
files, which it should not.

## Manual Testing Focus

No gameplay manual testing is expected for this docs-only phase. The manual review focus is whether
the registry is complete enough for a later executor to decide what goes into `GameState`,
`DerivedState`, transient runtime state, and compatibility metadata without rereading the whole
simulation.

## Handoff

The handoff must name:

- where the registry lives and how to update it when `Game` fields change;
- every field whose category remains uncertain, with the concrete code/design evidence still needed;
- any Phase 0.5 derived-state assumption that the inventory confirmed or contradicted;
- the exact docs-only verification command that passed;
- the recommended next phase, which should not begin code movement until the registry has no
  unresolved ownership blockers for current `Game` fields.
