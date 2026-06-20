# Phase 3 - Default Weapon Fit Policy

## Phase Status

- [ ] Not started.

## Objective

Use the ranking boundary to ship the first simple gameplay improvement: default weapons choose
targets that fit their weapon role. This phase should solve the immediate Rifleman/non-AP and
Tank/anti-armor priority requirements without adding alternate attacks or ability autocast.

## Work

- Replace the narrow `TargetPriority::PrefersArmored` concept with, or wrap it behind, a richer pure
  policy vocabulary:
  - attacker default weapon class;
  - target armor class;
  - target threat role, especially anti-armor threat;
  - target structure/field-obstacle role where needed.
- Keep the first vocabulary deliberately small. Prefer `WeaponClass`, `ArmorClass`, and a compact
  target/threat classifier over many per-unit priority rows.
- Implement small-arms default weapon fit:
  - Riflemen, Workers, Machine Gunners, Scout Cars, and other non-AP default attacks prefer soft
    targets over armored or hard targets;
  - soft preference is not target invalidation, so these units still attack armored/hard targets when
    no better legal target exists;
  - unit-over-building preference should remain compatible with weapon-fit preference.
- Implement anti-armor default weapon fit:
  - AP/anti-armor attackers prefer armored or hard targets over ordinary soft targets;
  - anti-armor threat targets outrank generic armored targets where appropriate;
  - Anti-Tank Guns remain high-value targets for Tanks.
- Generalize Tank priority:
  - Anti-Tank Guns remain the top target when legal and in relevant range;
  - other AP or anti-armor threats should outrank harmless soft targets;
  - Tanks, Tank Traps, and Mortar Teams should be ranked by named threat/fit terms, not a fixed
    Tank-only kind list where possible.
- Add tests with mixed candidate sets:
  - Rifleman chooses Rifleman/MachineGunner over Tank or Tank Trap when all are legal;
  - Rifleman falls back to Tank or Tank Trap when no soft target is legal;
  - Tank chooses Anti-Tank Gun over closer low-threat targets;
  - Tank chooses a generalized anti-armor threat over an ordinary soft target;
  - Anti-Tank Gun chooses Tank/armored target over infantry when both are legal;
  - distance and id tie-breaks remain deterministic inside equal-rank buckets.
- Update `docs/design/balance.md` and `docs/design/server-sim.md` to describe the first-iteration
  priority model and its limits.
- Collect factual patch-note bullets for player-facing behavior.

## Expected Touch Points

- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/combat.rs`
- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/tests.rs`
- `docs/design/balance.md`
- `docs/design/server-sim.md`

## Implementation Checklist

- [ ] Add or expose pure combat classification helpers for target ranking.
- [ ] Implement small-arms soft-target preference.
- [ ] Implement anti-armor and anti-armor-threat preference.
- [ ] Preserve Anti-Tank Gun as top Tank threat.
- [ ] Remove or isolate obsolete narrow priority branches after tests cover the new policy.
- [ ] Add mixed-target acquisition tests.
- [ ] Update design docs and patch-note bullets.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase as done in this file.

## Verification

```bash
cargo test --manifest-path server/Cargo.toml -p rts-rules combat
cargo test --manifest-path server/Cargo.toml -p rts-sim game::services::combat
node scripts/check-docs-health.mjs
git diff --check
```

If a client-visible stat, catalog, or wiki-visible balance table changes unexpectedly, also run:

```bash
node scripts/check-wiki.mjs
node scripts/check-faction-catalog-parity.mjs
```

## Manual Test Focus

Open a local match or dev scenario with Riflemen, Machine Gunners, Tanks, Anti-Tank Guns, and Tank
Traps visible at comparable ranges. Confirm small-arms units shoot soft targets before wasting fire
on armor, Tanks snap to Anti-Tank Guns over lower-threat targets, and units still fire at fallback
targets when no preferred target is available.

## Handoff Expectations

Report exact player-facing target-selection changes and any changed docs. The next agent should use
the new rank terms for retention/hysteresis in Phase 4 instead of adding separate target switching
logic.
