# Phase 9 - Docs, Data Surfaces, And Integration Hardening

## Phase Status

Status: pending.

## Objective

Close integration gaps after the refactors, runtime behavior, client feedback, and docs updates
land. This phase should make the finished feature understandable in source-of-truth docs and
generated references, then harden the full workflow with focused regression and manual scenario
coverage.

## Scope

- Audit the final implementation against every bullet in [requirements.md](requirements.md).
- Update `docs/design/server-sim.md` to describe weapon profiles, secondary Tank coax firing,
  independent cooldowns, target-policy activation constraints, arc gating, and panic-free stale
  target behavior.
- Update `docs/design/balance.md` to describe Tank coax range, damage, cooldown, weapon class,
  overpenetration, target priority, and unchanged Tank cost/supply/sight/trainability.
- Update `docs/design/protocol.md` if the final attack `weaponKind` field or compact schema differs
  from Phase 4 docs.
- Update `docs/design/client-ui.md` to describe weapon-specific attack feedback and Tank coax rig
  treatment if Phase 8 changed renderer contracts.
- Update generated stats/wiki surfaces if implementation exposes secondary weapons there. The Tank
  primary displayed range should remain the main-cannon range unless a later requirement adds a
  separate coax range display.
- Add or tighten focused integration tests for cannon/coax same-tick behavior, event ordering,
  replay stability, fog projection, client fallback behavior, and nearby combat regressions.
- Add a dev or lab scenario if existing scenarios do not make coax inspection practical.
- Resolve small documentation/product mismatches found during manual playtest or CI.
- Collect factual patch-note bullets for the final implementation.

## Out Of Scope

- No new coax gameplay tuning unless the user explicitly approves a balance change.
- No command-card button, toggle, upgrade, research, new range display, cost/supply/sight change, or
  trainability change.
- No broad unrelated refactors.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/balance.md`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/testing.md` only if a new scenario or verification workflow becomes a lasting
  contract
- `plans/coax/requirements.md` only for decisions made during implementation
- Focused Rust combat/projection/replay tests under `server/crates/sim/src/game/**`
- Focused client contract tests under `tests/client_contracts/**`
- Dev or lab scenario setup under `server/crates/sim/src/game/setup/dev_scenarios/**` if needed
- `server/crates/rules/src/bin/dump-faction-catalog.rs` or wiki data helpers if secondary weapons
  become generated
- `server/src/wiki*` or related wiki/stat generation files if applicable
- `client/src/config*.js` only if a consumed mirror is required

## Edge Cases To Cover

- Docs do not imply that Tank command card, cost, supply, sight, trainability, or primary range
  display changed.
- Docs distinguish Tank cannon AP behavior from coax small-arms behavior.
- Docs state that coax overpenetrates with small-arms damage.
- Protocol docs match actual Rust and JS attack-event weapon field names and compact slot shape.
- Wiki/generated stats either mention the secondary weapon accurately or intentionally omit it until
  secondary weapons are supported.
- Replays and spectator/lab projections show the same weapon-specific feedback as live views within
  their projection policy.
- Missing weapon identity remains stable for old fixtures and legacy events.
- Manual inspection can reproduce in-arc infantry priority, fallback vehicle/building shots, and
  no-fire outside arc.

## Verification

- Focused Rust integration tests for final coax behavior and nearby regressions.
- Focused client contract tests for final weapon-specific feedback.
- `node tests/protocol_parity.mjs`
- `node scripts/check-client-architecture.mjs` if client files are touched.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-docs-health.mjs`
- `node scripts/check-wiki.mjs` if generated stats/wiki surfaces are touched.
- `node scripts/check-faction-catalog-parity.mjs` if visible rules/catalog mirrors are touched.
- `git diff --check`

## Manual Test Focus

Run a local server and inspect a dev or lab scenario with Tanks, infantry-priority targets, Ekat,
Golems, support weapons, armored fallback targets, buildings, blockers, smoke, and resources.
Confirm the coax fires only through the turret arc, overpenetrates at small-arms scale, uses MG
feedback, and never makes the Tank turret or hull chase soft targets by itself.

## Handoff Expectations

Provide a final requirement-by-requirement checklist, verification commands, manual test notes, and
remaining watch items for playtests. Include factual patch notes suitable for the PR body and
release notes.
