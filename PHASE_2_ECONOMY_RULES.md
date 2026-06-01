# Phase 2 — Extract `rules/economy.rs`

## Goal

Move tech/production *rules* out of `config.rs` into `server/src/rules/economy.rs`. `config.rs` is left as a pure balance table (numbers only). **No behavior change.**

## Starting state assumptions

- Phase 1 is merged. `server/src/rules/mod.rs` and `server/src/rules/combat.rs` exist.
- `config.rs` still contains `trainable_units`, `build_requirement_met`, `train_requirement_met`, `node_amount`, and the `ww2_production_chain_matches_design` test (lines 271–373).
- Callers: `services/commands.rs:207,259` and any AI code that asks "what can I train / build right now?".

## Out of scope

- Changing costs, supply numbers, or `UnitStats` / `BuildingStats` layout — those stay in `config.rs`.
- Restructuring `unit_stats` / `building_stats` lookups (Phase 3).
- Refunds / partial credit on cancel — separate feature.

## Steps

1. **Create `server/src/rules/economy.rs`** and move:
   - `pub fn trainable_units(building_kind: EntityKind) -> &'static [EntityKind]`
   - `pub fn build_requirement_met(building_kind, owned_building_kinds) -> bool`
   - `pub fn train_requirement_met(unit_kind, owned_complete_building_kinds) -> bool`
   - `pub fn node_amount(kind: EntityKind) -> u32`

2. **Add thin wrappers** for cost/supply lookups that read from `config::unit_stats` / `config::building_stats` but expose them under the rules namespace, so service code stops importing `config` for rules questions:
   - `pub fn cost(kind: EntityKind) -> (u32, u32)` returning `(steel, oil)`.
   - `pub fn supply_cost(kind: EntityKind) -> u32` (units only; 0 otherwise).
   - `pub fn supply_provided(kind: EntityKind) -> u32` (buildings only).

   These are pure pass-throughs; do not duplicate numbers. They exist so that "what does a thing cost" is a rules call, not a `config::unit_stats(kind).unwrap().cost_steel` dance.

3. **Update `config.rs`:** delete the four moved functions and their test. `config.rs` should end up with only constants, `UnitStats`, `BuildingStats`, `unit_stats`, `building_stats`, `map_size_for`. Re-export nothing through `config` — callers update their imports.

4. **Update call sites:**
   - `services/commands.rs:207` → `rules::economy::build_requirement_met(...)`.
   - `services/commands.rs:259` → `rules::economy::train_requirement_met(...)`.
   - `services/production.rs` / `construction.rs` / `economy.rs` — anywhere that reads cost or supply from `config::unit_stats(kind).unwrap()` to gate an action, switch to `rules::economy::cost(...)` / `supply_cost(...)`. Leave reads of stats *for simulation* (e.g., `range_tiles`, `speed`, `hp`) in `config`.
   - AI code (`game/ai.rs`, `ai_shared.rs`) — same: rules calls for "can I afford / am I allowed", config calls for "what are the raw stats".

5. **Move the `ww2_production_chain_matches_design` test** out of `config.rs#tests` into `rules/economy.rs#tests`.

6. **Doc update:** extend the `§3.x Rules layer` paragraph from Phase 1 with the economy sub-module. State the rule: `config.rs` answers "what number?"; `rules/economy.rs` answers "is this allowed / what does it cost?".

## Done when

- `config.rs` has zero functions named `*requirement*`, `trainable_*`, `node_amount`.
- `git grep "config::build_requirement_met\|config::train_requirement_met\|config::trainable_units\|config::node_amount"` is empty.
- All tests green (cargo + three node scripts).
- `config.rs` line count drops by ~100; `rules/economy.rs` is a self-contained module under ~150 lines.
