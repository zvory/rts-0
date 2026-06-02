# AI-1: Shared World Model

Status: Done.

Build one deterministic way for AI code to ask "what is true right now?"

This phase should not substantially change behavior. It should extract observations and facts from
the existing live AI and self-play scripts so later phases can change decisions safely.

## Goal

Create a shared AI world-model layer that can be built from:

- authoritative live game state
- self-play `PlayerView`/`Snapshot`

The output should be stable, small, and strategy-oriented. Later code should not have to inspect
raw `EntityStore` or raw snapshot entities for common RTS questions.

## Suggested Files

- Add `server/src/game/ai_core/mod.rs`.
- Add `server/src/game/ai_core/observation.rs`.
- Add `server/src/game/ai_core/facts.rs`.
- Keep `server/src/game/ai_shared.rs` as a compatibility shim while migrating existing helpers.
- Update `server/src/game/mod.rs` to expose the new private module.

## Inputs to Preserve

Live AI currently has:

- `Map`
- `EntityStore`
- `SpatialIndex`
- `PlayerState`
- player id
- current tick

Self-play currently has:

- `PlayerView`
- `StartPayload`
- `Snapshot`
- player id
- current tick
- local script state such as pending builds and failed build spots

The shared facts layer should accept enough context to represent both cases.

## Core Data to Derive

Start with facts that already exist in duplicated form:

- own start tile
- own resource bank and supply
- own workers by order/disposition
- idle workers
- gathering workers
- build-capable workers
- committed workers already walking to build sites
- pending build intents
- buildings by kind, complete/incomplete/intended
- complete production buildings with queue lengths
- supply pressure and supply cap max status
- main-base steel saturation target
- known oil nodes relevant to tech profiles
- free combat units by kind
- staged, ready, and committed attack units where applicable
- known enemy start tiles
- nearest living enemy start tile

Do not add complex scouting memory in this phase. Leave last-known-enemy memory for later.

## Subtasks

### AI-1.1 Inventory Existing Queries

Use `rg` to find duplicated logic in:

- `server/src/game/ai.rs`
- `server/src/game/ai_shared.rs`
- `server/src/game/selfplay.rs`
- `server/src/game/services/world_query.rs`

Record only code comments if they help the extraction. Do not create a separate investigation doc
unless implementation reveals a large unknown.

### AI-1.2 Add Observation Types

Add small types that represent AI-visible inputs. Keep them boring.

Suggested shape:

- `AiObservation`
  - `player_id`
  - `tick`
  - map dimensions and tile size
  - own player economy/supply
  - own start tile
  - known enemy start tiles
  - owned entity summaries
  - visible or known resource summaries
  - visible enemy summaries if available

Use summaries, not raw entity references, when it simplifies testing and self-play reuse.

All vectors that affect decisions should be sorted by stable ids or stable coordinates.

### AI-1.3 Add Live Observation Adapter

Create a function that builds `AiObservation` from the authoritative live AI inputs.

This adapter may use authoritative state, but it should expose only the fields the decision core
needs. Preserve the current fairness rule: live AI should not use hidden enemy positions for
attacks until explicit scouting/memory work exists.

### AI-1.4 Add Self-Play Observation Adapter

Create a function that builds `AiObservation` from self-play `PlayerView`.

Do not move all self-play scripts in this phase. The goal is to make the adapter exist and prove it
can answer the common facts.

### AI-1.5 Add Facts Builder

Add `AiFacts` or equivalent derived facts on top of `AiObservation`.

Good first facts:

- `worker_count`
- `target_steel_workers`
- `free_supply`
- `supply_blocked_or_near_blocked`
- `depot_in_progress`
- `building_count(kind)`
- `complete_building_count(kind)`
- `production_buildings(kind)`
- `free_combat_units(kind)`
- `nearest_public_enemy_base`

Keep rules local and deterministic. If a fact needs real game rules such as costs or tech
requirements, call the canonical rule/definition helper available today. Do not copy constants.

### AI-1.6 Migrate Existing Pure Helpers

Move or wrap existing `ai_shared` helpers where they naturally fit:

- worker saturation target
- build-site search input shape
- attack-wave readiness

It is acceptable to leave `ai_shared` re-exporting or delegating during migration. Do not force a
large rename just to make the module tree pretty.

### AI-1.7 Add Unit Tests

Add focused tests for facts that can be built without a full match:

- steel saturation from entity view and snapshot view agree
- pending depot/build intent is counted once
- production queue facts are sorted and stable
- nearest enemy start tile selection is deterministic
- free combat unit selection ignores busy units

Prefer small fixtures over running a whole game.

## Non-Goals

- No new strategy behavior.
- No profile selection.
- No self-play script deletion.
- No protocol changes.
- No scouting memory.
- No adaptive opponent modeling.

## Validation

Run targeted Rust tests for the touched modules. If this phase only extracts pure helpers, full
self-play is useful but not required before committing.

## Done Criteria

AI-1 is done when:

- live AI and self-play can both construct the shared observation/facts types
- at least three duplicated facts are centralized
- existing live AI behavior is effectively unchanged
- tests cover the centralized facts
