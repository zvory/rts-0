# Phase 3 — Introduce `rules/defs.rs`

## Goal

Replace ad-hoc `match kind { ... }` blocks scattered across the codebase with a single data-driven definition table per kind, exposed as immutable `&'static` records. Rules functions (Phase 1/2) become thin readers over this table. **Behavior unchanged.** This phase is structural: it removes the "smear" so future additions (new unit, new weapon class) touch one record, not five files.

## Starting state assumptions

- Phases 1 and 2 are merged.
- `rules/combat.rs` and `rules/economy.rs` are populated with pure functions reading from `config::unit_stats` / `config::building_stats` and hardcoded classification matches (`is_armored`, `is_ap`, `prefers_armored_targets`, `trainable_units`, `*_requirement_met`).
- `EntityKind` is still the runtime identity of a thing.

## Out of scope

- Removing or renaming `EntityKind` — it stays as the integer-ish identity.
- Moving balance numbers to TOML/JSON — defs are still Rust `const`s for now.
- Terrain / cover / projection — Phase 4.

## Steps

1. **Create `server/src/rules/defs.rs`** with three record types and one table per:

   ```rust
   pub struct UnitDef {
       pub kind: EntityKind,
       pub stats: config::UnitStats,
       pub armor_class: ArmorClass,        // Soft | Armored
       pub weapon: WeaponClass,            // SmallArms | AntiTank | None
       pub target_priority: TargetPriority,// Default | PrefersArmored
       pub trained_at: Option<EntityKind>, // building that trains it
       pub train_requires: &'static [EntityKind],
   }

   pub struct BuildingDef {
       pub kind: EntityKind,
       pub stats: config::BuildingStats,
       pub armor_class: ArmorClass,
       pub weapon: WeaponClass,
       pub trains: &'static [EntityKind],
       pub build_requires: &'static [EntityKind],
   }

   pub struct NodeDef {
       pub kind: EntityKind,
       pub amount: u32,
   }
   ```

   Plus enums `ArmorClass { Soft, Armored }`, `WeaponClass { None, SmallArms, AntiTank }`, `TargetPriority { Default, PrefersArmored }`.

2. **Populate the tables** as `pub const UNITS: &[UnitDef] = &[ ... ]`, `BUILDINGS: &[BuildingDef]`, `NODES: &[NodeDef]`. Each entry reproduces today's behavior exactly:
   - `Tank`: armored + AT + default priority.
   - `AtTeam`: soft + AT + prefers-armored.
   - Buildings: armored + no weapon (today) — leaves room for future turrets without code restructure.
   - `trained_at` / `train_requires` / `build_requires` come from the current `trainable_units` / `*_requirement_met` matches.

3. **Add lookups:**
   - `pub fn unit_def(kind: EntityKind) -> Option<&'static UnitDef>`
   - `pub fn building_def(kind: EntityKind) -> Option<&'static BuildingDef>`
   - `pub fn node_def(kind: EntityKind) -> Option<&'static NodeDef>`

   Implement via linear scan of the static slice — table is tiny.

4. **Rewrite `rules/combat.rs` and `rules/economy.rs`** to delegate to defs:
   - `is_armored(kind) = unit_def(kind).map(|d| d.armor_class) | building_def(kind).map(...) == Armored`.
   - `is_ap(kind) = weapon(kind) == AntiTank`.
   - `prefers_armored_targets(kind) = unit_def(kind).map(|d| d.target_priority == PrefersArmored).unwrap_or(false)`.
   - `trainable_units(building) = building_def(building).map(|d| d.trains).unwrap_or(&[])`.
   - `build_requirement_met` / `train_requirement_met` iterate over the `*_requires` slice and check `owned.contains(req)`.
   - `node_amount` reads `node_def(kind).map(|d| d.amount).unwrap_or(0)`.

5. **Decide on `unit_stats` / `building_stats`:** keep them in `config.rs` *as the data source* the defs embed, OR move the literal `UnitStats { ... }` records into `defs.rs` and have `config::unit_stats(kind)` become `defs::unit_def(kind).map(|d| d.stats)`. Recommended: move them into `defs.rs` — `config.rs` then holds only scalar constants (timings, map sizes, starting amounts, splash slack, etc.). Pure data colocation, no behavior change.

6. **Tests:**
   - Add a `defs.rs#tests` consistency check: every `EntityKind` variant resolves to exactly one of unit/building/node def.
   - Add a test that `is_armored` / `is_ap` outputs over all `EntityKind` variants match the Phase 1 table byte-for-byte.
   - Keep the existing production-chain test.

7. **Doc update:** rewrite `DESIGN.md` §5 (balance table) to describe defs as the source of truth and `config.rs` as a thin constants module. Cross-reference §3.x rules layer.

## Done when

- Every classification match (`matches!(kind, EntityKind::X | ...)`) inside `server/src/` lives in `rules/defs.rs`. Other modules read via def lookups.
- Adding a hypothetical `Halftrack` unit requires editing exactly one location: append a `UnitDef` to `UNITS`. Verify by walking through the steps in a comment.
- All tests green.
