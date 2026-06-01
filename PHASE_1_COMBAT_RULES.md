# Phase 1 — Extract `rules/combat.rs`

## Goal

Create `server/src/rules/` and move all combat-classification + damage-formula logic out of `entity.rs` and `services/combat.rs` into pure functions there. **No behavior change.** Mechanical extraction only.

## Starting state assumptions

- Branch: `zvorygin/rules-layer` (worktree `../rts-rules-layer`).
- No `rules/` module exists yet.
- `EntityKind::is_armored` / `is_ap` live on the enum in `server/src/game/entity.rs:64,69`.
- `combat.rs` contains `attack_profile` (line ~228), `combat_mode` (~249), AT-team priority block (~289), AP/armor damage formula (~328-340 and ~431-445).

## Out of scope

- Touching economy, training, build requirements — that is Phase 2.
- Designing a data-driven defs table — Phase 3.
- Terrain, cover, concealment — Phase 4.
- Any tuning change.

## Steps

1. **Create `server/src/rules/mod.rs`** with module declaration and one-line doc tying it back to `DESIGN.md §3` (new rules section). Add `pub mod rules;` to `server/src/main.rs`.

2. **Create `server/src/rules/combat.rs`** and move:
   - `EntityKind::is_armored` → `pub fn is_armored(kind: EntityKind) -> bool`
   - `EntityKind::is_ap` → `pub fn is_ap(kind: EntityKind) -> bool`
   - `attack_profile(&Entity)` from `combat.rs:228` → `pub fn attack_profile(kind: EntityKind) -> AttackProfile { range_tiles, dmg, cooldown }` (struct, not tuple — clearer at call sites). Looks up `unit_stats` / `building_stats` from `config.rs` exactly as today.
   - Damage formula from `combat.rs:336-340` → `pub fn effective_damage(attacker_kind, victim_kind, base_dmg) -> u32`. Single helper, used by both direct and overpenetration paths.
   - AT-team target priority predicate → `pub fn prefers_armored_targets(kind: EntityKind) -> bool` (returns true for `AtTeam` today). Used by the AT-team branch in `resolve_target`.

3. **Leave `combat_mode` in `services/combat.rs`** for now — it reads `Order` state, not a pure rule of the unit kind. Phase 3 may revisit if it becomes data-driven.

4. **Update call sites:**
   - `services/combat.rs:228` — delete `attack_profile`, callers use `rules::combat::attack_profile(e.kind)`.
   - `services/combat.rs:328-340` — replace inline AP/armor branch with `rules::combat::effective_damage(...)`.
   - `services/combat.rs:430-445` — same replacement in overpenetration.
   - `services/combat.rs:289-301` — AT-team branch gated on `rules::combat::prefers_armored_targets(kind)` instead of hardcoded `kind == AtTeam`.
   - Remove `EntityKind::is_armored` / `is_ap` from `entity.rs`. Grep for any remaining external callers and redirect them through `rules::combat::*`.

5. **Tests:**
   - Move the AP/armor unit tests that currently live in `combat.rs#tests` (if any) into `rules/combat.rs#tests`. Add direct tests for `effective_damage` covering: AP vs armored, non-AP vs armored, AP vs unarmored, non-AP vs unarmored.
   - Confirm `cargo test` passes and the three node test scripts (`server_integration.mjs`, `regression.mjs`, `ai_integration.mjs`) still pass against a running server.

6. **Doc update:** add a short `§3.x Rules layer` paragraph in `DESIGN.md` describing the seam: rules functions are pure, take `EntityKind` + context primitives, never mutate, never read fog. Services orchestrate; rules classify.

## Done when

- `git grep is_armored\|is_ap` only shows hits inside `server/src/rules/combat.rs` and its callers.
- `server/src/rules/combat.rs` has zero imports from `services/` or `entity.rs` beyond the `EntityKind` enum.
- All tests green.
- Diff is move + rename + call-site update. No new behavior, no new abstractions.
