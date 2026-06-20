# Phase 1 - Baseline Contract Tests

## Phase Status

- [ ] Not started.

## Objective

Add focused regression coverage for current combat acquisition behavior before changing the target
selection architecture. This phase should make the current contracts explicit enough that Phase 2 can
move acquisition into a ranking boundary without accidental gameplay changes.

## Work

- Add or tighten tests around `server/crates/sim/src/game/services/combat/acquisition.rs` and the
  combat system fixtures for current target selection.
- Cover explicit `Attack` order preservation:
  - explicit targets are retained while enemy, targetable, visible, and fireable;
  - explicit targets are dropped when invalid, then normal acquisition may run;
  - explicit attacks against visible enemy Tank Traps remain legal for infantry-like units.
- Cover current Tank priority behavior:
  - Anti-Tank Gun outranks Tank, Tank Trap, Mortar Team, and Rifleman when in weapon range;
  - an out-of-weapon-range priority target does not steal focus from a valid in-range target;
  - a higher-priority Tank target can override a retained lower-priority moving-fire target.
- Cover Anti-Tank Gun and anti-armor baseline behavior:
  - the current `TargetPriority::PrefersArmored` path prefers Tanks over ordinary soft targets;
  - fog, smoke, and line-of-sight rejection still prevent hidden or blocked targets.
- Cover small-arms baseline behavior:
  - current unit-over-building preference is documented by tests;
  - small-arms units can still fall back to armored or hard targets when no better legal target
    exists, so Phase 3 can distinguish preference from target invalidation.
- Cover Tank Trap baseline behavior:
  - infantry-like auto-acquisition ignores enemy Tank Traps;
  - vehicle-body auto-acquisition can target enemy Tank Traps;
  - Tank Traps do not block direct-fire shots at a valid unit behind them;
  - own/allied Tank Traps are not hostile targets.
- Add any pure `rts-rules` tests needed to pin existing `ArmorClass`, `WeaponClass`, AP, armor, and
  current target-priority classification facts.
- Do not refactor production code except for tiny test-only helpers if needed.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/tests.rs`
- `server/crates/rules/src/combat.rs`
- possibly local test helpers in `server/crates/sim/src/game/services/combat/tests.rs`

## Implementation Checklist

- [ ] Add explicit-attack preservation tests.
- [ ] Add Tank priority and retained-target override tests.
- [ ] Add Anti-Tank Gun armored-preference tests.
- [ ] Add small-arms unit/building and armored-fallback baseline tests.
- [ ] Add Tank Trap acquisition and shot-transparency baseline tests if gaps remain.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase as done in this file.

## Verification

```bash
cargo test --manifest-path server/Cargo.toml -p rts-rules combat
cargo test --manifest-path server/Cargo.toml -p rts-sim game::services::combat
git diff --check
```

If documentation changes, also run:

```bash
node scripts/check-docs-health.mjs
```

## Manual Test Focus

No manual gameplay test is required for this test-only phase unless a fixture exposes surprising
behavior. If the phase reveals a mismatch between expected and actual behavior, report the exact
scenario and do not change gameplay in this phase.

## Handoff Expectations

Report which current contracts are now covered, which behaviors remain intentionally undocumented by
tests, and any surprising current behavior found during test writing. The next agent should use
these tests as the no-gameplay-change gate for Phase 2.
