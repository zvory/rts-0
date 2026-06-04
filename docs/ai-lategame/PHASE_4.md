# Phase 4 - Regression Coverage And Documentation Audit

## Goal

Lock down the AI late-game convergence behavior with focused tests and update player/developer docs
to match the implemented behavior.

This phase should make future tuning safer. If someone edits one profile's late game by accident,
tests should catch the drift.

## Plain-Language Intent

After the refactor, the important promise is simple:

- openings differ;
- late game converges;
- all live profiles can expand eventually.

The tests should say that directly. They should not require a full 20,000-tick self-play run just
to prove profile data is wired correctly.

## Test Coverage Targets

### Profile Pool Tests

Prove the live lobby pool still contains the intended profiles:

- `tech_to_tanks`;
- `rifle_flood_fast`;
- `rifle_flood_full_saturation`.

Also prove `steel_expansion_tanks` remains available to test/self-play tooling if that is still
desired, without accidentally adding it to the live pool.

### Opening Identity Tests

Prove early behavior remains distinct:

- `rifle_flood_fast` still has proxy barracks policy and early rifle pressure;
- `rifle_flood_full_saturation` still has a macro rifle opening;
- `tech_to_tanks` still requires tank tech early.

These tests should be narrow profile-data tests where possible.

### Late-Game Convergence Tests

Prove late-game behavior is shared:

- live profiles use the same late-game production priorities;
- live profiles use the same late-game attack unit kinds;
- live profiles use the same tank-required attack rule;
- wave sizes, wave growth, regroup reset, reissue cadence, and staging distance match the shared
  policy;
- transition logic selects shared late game after the configured trigger.

### Expansion Tests

Prove expansion expectations are shared:

- every live profile has an expansion policy;
- every live profile targets at least 2 City Centres eventually;
- expansion policy does not block tank tech for live profiles unless explicitly documented;
- shared expansion constants are not silently forked.

### Decision Output Tests

Add small decision-level tests where profile-data tests are not enough:

- before transition, a rifle profile selects rifle-only ready units for outbound attacks;
- after transition, the same profile selects the shared tank+rifle attack policy;
- a tank-required late-game attack does not launch without a tank;
- when a tank is ready and the wave threshold is met, the decision emits an outbound `AttackMove`.

Do not add tank-front/rifle-back command-shape tests. That behavior is intentionally out of scope.

## Documentation Updates

Update `DESIGN.md` section 8 after implementation, not before. It should describe:

- live AIs still choose one profile at match start;
- openings remain profile-specific;
- late-game tank production and outbound wave behavior is shared;
- expansion is an eventual expectation across live profiles;
- `rifle_flood_fast` may still need separate recovery tuning if it fails to convert early pressure.

If implementation changes any balance-relevant behavior, collect patch-note bullets:

- changed late-game wave size;
- changed tank/rifle production mix;
- changed expansion trigger;
- changed profile transition trigger;
- changed likelihood of one-base all-in behavior.

## Suggested Test Locations

Likely places:

- `server/src/game/ai_core/profiles.rs` for pure policy tests;
- `server/src/game/ai_core/decision.rs` for active-policy and command-output tests;
- `server/src/game/mod.rs` for live profile pool selection tests if they already live there.

Avoid broad integration tests unless the unit-level tests cannot observe the behavior. Broad
self-play is useful as a confidence check, but it should not be the only guard.

## Expected Behavior At End

At the end:

- future AI profile edits fail tests if they accidentally fork late-game policy;
- docs match the implemented convergence model;
- player-facing patch notes describe the actual gameplay impact;
- no formation behavior is claimed or tested;
- `rifle_flood_fast` recovery remains a known separate follow-up.

## Done When

- Tests prove opening identity and late-game convergence.
- Tests prove all live profiles have eventual expansion paths.
- `DESIGN.md` accurately reflects implemented behavior.
- Patch-note bullets are ready for the commit/PR summary if behavior changed.
