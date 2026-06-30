# Phase 7 - Integration And Playtest Hardening

## Phase Status

Status: pending.

## Objective

Close integration gaps after the refactor, server runtime, client feedback, and docs phases land.
This phase should focus on regressions, replay/fog confidence, manual playtest setup, and any small
hardening fixes discovered by CI or local inspection.

## Scope

- Audit the final implementation against every bullet in [requirements.md](requirements.md).
- Add or tighten focused tests for cross-cutting behavior that was awkward to cover in individual
  phases, especially cannon/coax same-tick interactions, event ordering, replay stability, and fog
  projection.
- Add a dev scenario or lab scenario if existing scenarios do not make coax inspection practical.
- Verify that attack-event weapon identity remains fog-safe and does not expose hidden target data
  beyond current attack-event projection rules.
- Verify that old/default attacks, point fire, artillery, mortar, support weapons, Tank Traps,
  entrenchment, and moving-fire regressions are not broken by the weapon-profile refactor.
- Review file-size and architecture ratchets affected by the implementation and fix or bless only
  with a specific reason.
- Resolve small product/documentation mismatches found during manual playtest or CI.
- Do not introduce new coax gameplay tuning unless the user explicitly approves a balance change.

## Expected Touch Points

- Focused Rust combat/projection/replay tests under `server/crates/sim/src/game/**`
- Focused client contract tests under `tests/client_contracts/**`
- Dev or lab scenario setup under `server/crates/sim/src/game/setup/dev_scenarios/**` if needed
- `docs/design/testing.md` only if a new scenario or verification workflow becomes a lasting
  contract
- Any small source/doc fixes needed to make the fully integrated feature coherent

## Edge Cases To Cover

- Tank cannon and coax can operate independently without cooldown bleed-through.
- Coax does not fire through fog, smoke, blockers, resources, allies, or targets outside arc/range.
- Coax fallback targets and overpenetration use small-arms damage.
- Existing cannon firing reveal and overpenetration behavior are unchanged.
- Replays and spectator/lab projections show the same weapon-specific feedback as live views within
  their projection policy.
- Client fallback for missing weapon identity remains stable for old fixtures and legacy events.
- Manual inspection can reproduce in-arc infantry priority, fallback vehicle/building shots, and
  no-fire outside arc.

## Verification

- Focused Rust integration tests for final coax behavior and nearby regressions.
- Focused client contract tests for final weapon-specific feedback.
- `node tests/protocol_parity.mjs`.
- `node scripts/check-client-architecture.mjs` if client files are touched.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`.
- `node scripts/check-docs-health.mjs` if docs are touched.
- `git diff --check`.

## Manual Test Focus

Run a local server and inspect a dev or lab scenario with Tanks, infantry-priority targets, Ekat,
armored fallback targets, buildings, blockers, smoke, and resources. Confirm the coax fires only
through the turret arc, overpenetrates at small-arms scale, uses MG feedback, and never makes the
Tank turret or hull chase soft targets by itself.

## Handoff Expectations

Provide a final requirement-by-requirement checklist, verification commands, manual test notes, and
any remaining watch items for playtests. Include factual patch notes suitable for the PR body and
release notes.
