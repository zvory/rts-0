# Phase 2 - Ranking Boundary Without Gameplay Change

## Phase Status

- [ ] Not started.

## Objective

Introduce a named combat target ranking boundary while preserving current acquisition behavior. This
phase should make target selection easier to extend without yet changing what targets units choose.

## Work

- Add a sim-local module such as `server/crates/sim/src/game/services/combat/priority.rs`.
- Define a candidate shape that carries only already-legal target facts needed for ranking:
  - target id, kind, owner, position, distance squared;
  - unit/building classification;
  - armor class and weapon/threat classification from rules helpers;
  - Tank Trap relevance facts that exist today, without adding new obstruction behavior yet;
  - retained-target marker when the candidate is the attacker's current target.
- Define a deterministic rank shape with named terms. The first version should encode the current
  behavior, not the desired final behavior:
  - ordered attacks are handled before ranking as they are today;
  - Tank priority order still wins for in-weapon-range Tank targets;
  - moving-fire retained target behavior stays as-is except where existing Tank priority already
    overrides it;
  - Anti-Tank Gun `PrefersArmored` still means nearest Tank preference;
  - units still prefer enemy units over buildings;
  - nearest target wins inside the current fallback buckets.
- Replace repeated nearest-kind scans in acquisition with one legal-candidate collection path plus
  ranking where practical. If a full one-pass replacement is too risky, create the ranking boundary
  first and migrate only the clearly covered branches, but document what remains procedural.
- Keep legal filtering in acquisition/world-query helpers. Do not let ranking decide visibility,
  smoke, LOS, friendly-blocker, enemy targetability, or acquisition radius.
- Add tests that compare Phase 2 ranking outcomes with the Phase 1 baseline scenarios.
- Keep any new rules helpers pure. If the rank code needs armor or weapon class, expose simple
  read-only helpers from `rts-rules`; do not pass sim state into `rts-rules`.
- Update `docs/design/server-sim.md` if the acquisition ownership boundary materially changes.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/services/world_query.rs` if a ranked candidate iterator belongs there
- `server/crates/rules/src/combat.rs` for pure classification helpers only
- `docs/design/server-sim.md` if the combat service documentation changes

## Implementation Checklist

- [ ] Add the ranking/candidate module.
- [ ] Keep legal target filtering separate from ranking.
- [ ] Migrate current Tank priority into a named rank term or isolated policy function.
- [ ] Migrate current Anti-Tank Gun preference into a named rank term or isolated policy function.
- [ ] Migrate unit-over-building and nearest fallback behavior into ranking.
- [ ] Preserve moving-fire retained-target semantics.
- [ ] Add regression tests proving Phase 1 scenarios still pass.
- [ ] Update design docs if the combat acquisition boundary changes.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase as done in this file.

## Verification

```bash
cargo test --manifest-path server/Cargo.toml -p rts-rules combat
cargo test --manifest-path server/Cargo.toml -p rts-sim game::services::combat
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check
```

If docs change, also run:

```bash
node scripts/check-docs-health.mjs
```

## Manual Test Focus

Run or inspect a local skirmish with Riflemen, Tanks, Anti-Tank Guns, and Tank Traps. The expected
manual result is boring: units should appear to choose the same targets as before Phase 2.

## Handoff Expectations

Report the new module/function names, which acquisition branches were migrated, and any procedural
targeting logic that remains. The next agent should treat this phase as an architecture seam and
make Phase 3's gameplay changes through the ranking terms, not by adding new branches to
`resolve_target`.
