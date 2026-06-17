# Phase 1 - Scenario Matrix and Regression Harness

Status: done.

## Goal

Create the scenario and regression-test matrix for Tank Trap owner-aware vehicle pathing and
infantry auto-acquisition behavior before changing implementation semantics. The result should make
the desired behavior executable and inspectable enough that Phase 2 can be judged mechanically.

## Scope

- Add server-side dev scenarios for prebuilt Tank Trap walls instead of requiring manual
  construction before inspection.
- Add focused Rust regression coverage for target selection and pathing policy expectations where
  the current code allows it without implementing Phase 2 behavior early.
- Cover friendly/allied vehicle rerouting, enemy vehicle breach behavior, infantry pass-through,
  explicit infantry attack preservation, and charged-rifleman direct-order behavior.
- Extend testing documentation if new dev scenario ids or scenario launch options are added.
- Keep existing player-facing gameplay unchanged except for new local dev/test scenario entry
  points.

## Scenario Requirements

Add or extend game-backed dev scenarios under:

- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/layouts/tank_traps.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/tests.rs`
- `server/src/main.rs` dev scenario route/index code if new ids or query parameters are needed
- `docs/context/testing.md` if scenario ids change

Use one matrix scenario id with explicit case values so manual inspection stays grouped on the dev
scenario page:

- id: `tank_trap_pathing_matrix`
- cases: `friendly_vehicle_reroute`, `enemy_vehicle_breach`, `infantry_pass_through`,
  `explicit_infantry_attack`

The scenarios should be no-fog watcher rooms like the existing Tank Trap line scenarios. They
should spawn complete Tank Traps directly so Phase 2 can inspect pathing and combat behavior
without a manual build step.

## Automated Matrix

Add focused tests that assert the following, using existing sim helpers where possible:

- Own/allied Tank Traps are considered blockers for vehicle-body path planning.
- Enemy Tank Traps are represented in the scenario/harness as breachable obstacles that Phase 2
  will remove from the vehicle static path blocker layer.
- Tanks and Scout Cars moving into an enemy wall produce attack events against enemy Tank Traps and
  eventually make forward progress after destroying enough traps.
- Riflemen, Machine Gunners, and Workers crossing enemy Tank Traps do not auto-target the traps,
  do not emit attack events against them, and do not stop or set up because of them.
- Riflemen with Methamphetamines charge still do not auto-acquire enemy Tank Traps on move or
  attack-move orders.
- A directly ordered Rifleman attack against a visible enemy Tank Trap remains valid. When the
  Rifleman is charged, preserve existing moving-fire behavior if it can shoot while continuing
  through the trap line.
- Infantry-like units never attack own or allied Tank Traps because those traps are not hostile
  targets.

If a case cannot be expressed as a passing test before Phase 2, add it as a named pending/failing
expectation only if the repo already has a pattern for that. Otherwise document it in the phase
handoff and keep the Phase 2 test work explicit.

## Expected Touch Points

- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/layouts/tank_traps.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/tests.rs`
- `server/crates/sim/src/game/services/combat/tests.rs`
- `server/crates/sim/src/game/services/pathing.rs`
- `server/crates/sim/src/game/services/movement/tests.rs`
- `docs/context/testing.md`

## Out of Scope

- Do not change owner-aware pathing behavior in this phase.
- Do not change combat acquisition behavior in this phase.
- Do not change balance, HP, damage, construction, placement, or line-building behavior.
- Do not add client UI for these scenarios beyond whatever the dev scenario index already needs.

## Verification

Run the smallest focused checks that cover the new tests and scenario construction. Likely commands:

```bash
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -E 'package(rts-sim) & test(dev_scenario)'
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -E 'package(rts-sim) & test(tank_trap)'
```

If the implementation adds or changes the dev scenario index route, also run the focused server
route tests that cover `/dev/scenarios`.

## Manual Testing Focus

Start a local server and open the new scenario ids from `/dev/scenarios`. Confirm the scenarios are
legible: the wall, unit start, intended goal, and expected path/attack behavior are obvious without
needing to build traps manually.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must list the final scenario ids,
the exact focused test commands and results, any expected failures intentionally left for Phase 2,
and the core manual scenario URLs the Phase 2 agent should open after implementing behavior.

## Phase 1 Handoff

Final dev scenario matrix:

- id: `tank_trap_pathing_matrix`
- cases: `friendly_vehicle_reroute`, `enemy_vehicle_breach`, `infantry_pass_through`,
  `explicit_infantry_attack`

Focused verification added for scenario construction, `/dev/scenarios` index coverage, own/allied
vehicle blocker occupancy, current enemy-trap vehicle blocker representation, infantry move-order
pass-through without auto-attacks, and explicit Rifleman attack orders against visible enemy Tank
Traps.

Verification results:

- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -E 'package(rts-sim) & test(dev_scenario)'` could not run locally because `cargo-nextest` is not installed.
- `cargo test --manifest-path server/Cargo.toml -p rts-sim dev_scenario` passed: 24 tests.
- `cargo test --manifest-path server/Cargo.toml -p rts-sim tank_trap` passed: 23 tests.
- `cargo test --manifest-path server/Cargo.toml -p rts-server scenario` passed: 5 tests across the server lib and binary test targets.
- `node tests/client_contracts.mjs` passed.

Intentionally left for Phase 2:

- Enemy Tank Traps are still in the vehicle static blocker layer, so vehicle breach/progress tests
  remain a Phase 2 behavior target.
- Infantry attack-move still uses current combat acquisition semantics; Phase 2 should add the
  auto-acquisition filter and then make attack-move coverage pass for Riflemen, Machine Gunners,
  Workers, and the applicable moving-fire case.
- The legacy Charge command is currently a no-op in this branch, so charged Rifleman direct-order
  behavior could not be represented as a distinct passing Phase 1 regression.

Core manual URLs:

- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=scout_car&count=1&case=friendly_vehicle_reroute`
- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=tank&count=1&case=friendly_vehicle_reroute`
- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=scout_car&count=1&case=enemy_vehicle_breach`
- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=tank&count=1&case=enemy_vehicle_breach`
- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=rifleman&count=1&case=infantry_pass_through`
- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=machine_gunner&count=1&case=infantry_pass_through`
- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=worker&count=1&case=infantry_pass_through`
- `/dev/scenarios?id=tank_trap_pathing_matrix&unit=rifleman&count=1&case=explicit_infantry_attack`
