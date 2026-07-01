# Phase 7 - Integration Regression And Playtest Readiness

## Phase Status

Status: pending.

## Objective

Add the final focused regression coverage and manual playtest scaffolding for the fully integrated
Scout Plane. This phase should prove the complete normal-match feature against the requirements
without adding new product behavior or balance tuning.

## Scope

- Read [docs/context/testing.md](../../docs/context/testing.md) before deciding the final suite set.
- Audit the implementation against every bullet in [requirements.md](requirements.md).
- Add or tighten focused tests for:
  - City Centre button lock/unlock and disabled reason.
  - resource cost, build time, and 0 supply.
  - one active or in-production plane per player.
  - active select/pan behavior.
  - production cancel/refund and destroyed-producer interruption.
  - launch from City Centre and first-rally/no-rally behavior.
  - 2 px/tick movement and 4-tile orbit radius.
  - move retargeting and queued retargeting.
  - no combat, no targeting, no repair, no harvest, no garrison, no collision, no occupancy blocking.
  - oil upkeep cadence, reserve drain, reserve refill, auto-dismiss, and manual dismiss.
  - 12-tile aerial vision through terrain/building blockers.
  - smoke blocking.
  - enemy projection only while currently visible.
  - replay, checkpoint, lab, spectator, and observer projection safety.
  - client selection, mixed commands, command card, hotkeys, rendering, minimap, and teardown.
  - AI exclusion.
- Add or update a dev scenario, lab setup, or documented manual route that makes Scout Plane review
  quick.
- Use test selection evidence rather than running broad local bundles by default; rely on PR CI for
  the full `./tests/run-all.sh` gate.
- Do not tune Scout Plane cost, build time, sight, upkeep, speed, orbit radius, or unlock timing in
  this phase unless the user explicitly authorizes a balance change.

## Expected Touch Points

- `server/crates/sim/src/game/services/commands/tests/*.rs`
- `server/crates/sim/src/game/services/production.rs` tests
- `server/crates/sim/src/game/fog/tests.rs`
- `server/crates/sim/src/rules/projection.rs` tests
- `server/crates/sim/src/game/*checkpoint*` tests
- `server/crates/ai/src/` tests
- `server/src/lab_scenarios.rs`
- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `tests/client_contracts/*.mjs`
- `tests/hud_command_card.mjs`
- `tests/minimap_input_contracts.mjs`
- `tests/server_integration.mjs` only if the live pipeline needs new coverage
- `tests/regression.mjs` only if hardening/protocol projection coverage belongs there
- `tests/select-suites.mjs` if suite selection rules need updating
- `docs/design/testing.md` only if test workflow or scenario contracts change

## Edge Cases To Cover

- Requirements audit has no unchecked or ambiguous implementation bullets.
- A normal player can build exactly one Scout Plane and cannot bypass the cap through multiple City
  Centres, repeated hotkeys, queued commands, reconnects, or malformed client commands.
- Fog/projection tests cover both owner/team reveal and enemy non-leak cases.
- Upkeep and dismissal tests cover both zero-Oil failure and Oil-income recovery.
- Lab/dev/replay inspection path is documented and loads without manual setup surprises.
- Client teardown test or smoke coverage catches repeated match/rematch rendering resource leaks.
- AI exclusion is tested without requiring full long-running AI self-play unless implementation made
  AI encounter behavior risky.

## Verification

- `node tests/select-suites.mjs --from=origin/main` to confirm changed-file suite selection.
- Focused Rust and Node commands selected by the changed files.
- `node scripts/check-client-architecture.mjs` if client files changed.
- `node scripts/check-faction-catalog-parity.mjs` and `node scripts/check-wiki.mjs` if visible
  catalog/config/wiki surfaces changed.
- `node tests/protocol_parity.mjs` if protocol vocabulary changed.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  service boundaries changed.
- `node scripts/check-docs-health.mjs` if docs changed.
- `git diff --check`.

## Manual Test Focus

Run the documented Scout Plane inspection path and one normal local match. In the normal match,
verify locked City Centre button, unlock through Gun Works or Vehicle Works, production, launch to
rally, orbit retargeting, queued retargeting, oil drain/refill, manual dismiss, active select/pan,
enemy visibility near fog, and no movement blocking for ground units.

## Handoff Expectations

Name every focused test or suite added, the manual review route, any skipped suite with the concrete
reason, and every requirement bullet that remains deferred or intentionally out of scope. Tell Phase
8 what docs, generated references, patch notes, or review-package items still need final alignment.
