# Phase 4 — `rules/terrain.rs` and `rules/projection.rs`

## Goal

Add the two remaining rules sub-modules called out in the original architecture sketch. Unlike Phases 1–3, this phase **introduces structure ahead of features** — the modules start nearly empty, with stubs whose only job is to centralize where future terrain/cover logic and fog-projection logic will live. This phase is only worth doing once at least one consumer (e.g., a forest tile, a cover modifier, or a refined snapshot policy) is on the near-term roadmap.

## Gating decision

Before starting Phase 4, confirm one of:
- A terrain feature is the next planned work (forests, roads, elevation, cover) — Phase 4 lands as scaffolding for that feature.
- A snapshot/fog policy change is planned (e.g., last-known-position memory, partial unit-type reveal) — `projection.rs` exists for that.

If neither is true, **skip Phase 4**. Premature abstraction is worse than the smear.

## Starting state assumptions

- Phases 1–3 are merged. `rules/{combat,economy,defs}.rs` exist.
- Map terrain today is a uniform walkable grid; there is no per-tile cover or movement-class data. `game/map.rs` is purely tiles + footprints.
- Fog filtering currently lives in `game/fog.rs` and the per-player snapshot path in `game/mod.rs` / `services/world_query.rs`.

## Out of scope

- Implementing the actual terrain types or cover modifiers — that is the follow-up feature work.
- Changing the wire protocol — `EntityView` shape is unchanged.
- Last-known-position memory — flagged as a future hook only.

## Steps

### `rules/terrain.rs`

1. Define `TerrainKind` enum: `Open` (today's default). Reserve `Forest`, `Road`, `Hill` as commented-out variants for the follow-up feature.
2. Add three pure functions (all return today's defaults until terrain lands):
   - `pub fn movement_allowed(_kind: EntityKind, _terrain: TerrainKind) -> bool { true }`
   - `pub fn cover_modifier(_kind: EntityKind, _terrain: TerrainKind) -> f32 { 1.0 }` — multiplier on incoming damage.
   - `pub fn concealment_modifier(_kind: EntityKind, _terrain: TerrainKind) -> f32 { 1.0 }` — multiplier on enemy detection range against this unit.
3. **Wire them into combat:** `rules::combat::effective_damage(...)` accepts an optional `victim_terrain: TerrainKind` (default `Open` at call sites for now) and multiplies by `cover_modifier`. Same for fog acquisition radius via `concealment_modifier`. With the defaults above, behavior is identical — the seam is now in place.
4. **Wire into movement / pathing:** `services/pathing.rs` calls `terrain::movement_allowed(kind, Open)` per tile. Identical behavior, but the future "tank can't enter forest tile" rule has exactly one place to grow.

### `rules/projection.rs`

1. Move the per-player visibility predicates currently scattered through `services/world_query.rs` (e.g., `is_visible_to_player`) and `game/mod.rs` (snapshot construction's "should this entity appear for this player?") into named functions:
   - `pub fn entity_visible_to(viewer: u32, entity: &Entity, fog: &Fog) -> bool`
   - `pub fn event_visible_to(viewer: u32, event_origin_x: f32, y: f32, attacker_owner: u32, fog: &Fog) -> bool` — generalizes the duplicated visibility check in `apply_damage` and `apply_overpenetration`.
   - `pub fn project_entity(viewer: u32, entity: &Entity, fog: &Fog) -> Option<EntityView>` — wraps the snapshot builder for a single entity. Today identical to the existing builder; future "you see a tank silhouette but not its hp" lives here.
2. Reduce duplication in `services/combat.rs:360-372` and `combat.rs:452-463` (the two near-identical fog-gated event broadcast loops) to a single call into `projection`.
3. **No new entity views**, no new wire fields. Today's snapshot output must be byte-equal to before.

### Tests

- Add a snapshot-equivalence test: feed a fixed scenario through the old code path (via a git tag of the pre-Phase-4 commit, or by exercising both code paths in CI) and confirm identical `EntityView` lists. The simplest version: lock in current `selfplay` replay digests before Phase 4, rerun after, expect bit-for-bit match.
- Terrain stubs: unit tests on `terrain::*` confirming the trivial defaults — guards against accidental behavior drift while features land.

### Doc update

- `DESIGN.md` §3.x rules layer: extend to four sub-modules. Note that `terrain` and `projection` are intentionally near-empty seams, not abstractions for their own sake.
- `DESIGN.md` §6 (fog) and §5 (balance) cross-reference `projection.rs` and `terrain.rs` respectively.

## Done when

- `rules/` contains `mod.rs`, `combat.rs`, `economy.rs`, `defs.rs`, `terrain.rs`, `projection.rs`.
- All visibility checks inside `services/combat.rs` and snapshot construction go through `projection::*`.
- Pathing and combat call `terrain::*` at every tile-or-target read, even though the functions return defaults.
- All tests green; selfplay digests match the pre-Phase-4 baseline.
- The follow-up terrain feature has exactly one rules file to edit.
