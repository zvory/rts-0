# Phase 4 - Retargeting And Future Attack Profile Guardrails

## Phase Status

- [ ] Not started.

## Objective

Make target retention explicit and document the boundary between default weapon priority and future
alternate attacks. This phase should prevent the new priority system from thrashing targets or
accidentally becoming an ability planner when grenades and satchels are added later.

## Work

- Define a small retargeting rule in the ranking boundary:
  - keep the current valid target when the new best target is equal-rank or only trivially better;
  - switch immediately when a materially higher-rank threat appears, such as an Anti-Tank Gun for a
    Tank;
  - preserve existing ordered-attack behavior separately from auto-acquisition retention;
  - preserve moving-fire semantics for Tanks, Scout Cars, and charged Riflemen.
- Add tests for target stability:
  - equal-rank nearby targets do not cause oscillation every tick;
  - id/distance tie-breaks remain deterministic on first acquisition;
  - a materially higher-rank target steals focus;
  - retained targets are cleared when hidden, smoke-covered, dead, friendly, or no longer fireable.
- Document default attack profile scope:
  - the first ranking system chooses a target for the current default attack only;
  - future grenades, satchels, and melee demolition attacks must be represented as separate profiles
    with explicit activation policy;
  - explicit-only special attacks can be added without changing auto-acquisition;
  - future autocast must have its own conservative plan and tests.
- If helpful, add small type names or comments in the ranking module that reserve the profile
  boundary without implementing alternate profiles yet. Avoid dead abstractions; add only what makes
  the current code clearer.
- Update docs with the retargeting and future-profile boundary.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/tests.rs`
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Implementation Checklist

- [ ] Add a documented retargeting/stickiness rule.
- [ ] Preserve explicit ordered attack semantics outside rank-based retention.
- [ ] Add equal-rank stability tests.
- [ ] Add high-rank threat override tests.
- [ ] Add invalid-retained-target clearing tests.
- [ ] Document the default-profile versus future special-attack boundary.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase as done in this file.

## Verification

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim game::services::combat
node scripts/check-docs-health.mjs
git diff --check
```

If pure rules helpers change, also run:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-rules combat
```

## Manual Test Focus

Watch a small local fight where several equal Riflemen move in and out of range, then introduce a
higher-priority threat such as an Anti-Tank Gun near a Tank. Confirm units do not visually flicker
between equal targets every tick, but do switch when a meaningful threat appears.

## Handoff Expectations

Report the exact target-retention rule in plain language and point to the tests that protect it. The
next agent should use this stable rank/retention model when adding Tank Trap obstruction context in
Phase 5.
