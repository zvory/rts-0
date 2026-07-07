# Phase 8 - Docs, Patch Notes, And Review Package

## Phase Status

Status: done.

Completion notes:

- Audited `requirements.md` against the landed implementation and added final status, review route,
  patch-note, and deferred-item notes.
- Corrected stale contract wording in protocol, balance, server-sim, and client UI docs so Scout
  Plane production and selected-plane controls are described as current normal-match behavior.
- No gameplay behavior, balance values, protocol shapes, generated catalog data, or AI logic changed
  in this phase.

## Objective

Close documentation drift and prepare the final review package for the Scout Plane feature. This is
a final audit and review-readiness pass, not a place to add new gameplay behavior.

## Scope

- Audit [requirements.md](requirements.md) against the landed implementation and mark factual status,
  deferred items, or follow-up needs without rewriting approved behavior casually.
- Confirm contract docs are current:
  - [docs/design/protocol.md](../../../docs/design/protocol.md) for any wire, snapshot, command,
    projection, replay, lab, or compact transport changes.
  - [docs/design/balance.md](../../../docs/design/balance.md) for visible cost, build time, sight,
    speed, upkeep, active limit, and catalog surfaces.
  - [docs/design/server-sim.md](../../../docs/design/server-sim.md) for `Game` state ownership,
    checkpoint/replay state, fog rules, or `Game` API changes.
  - [docs/design/client-ui.md](../../../docs/design/client-ui.md) for HUD, input, rendering, lab,
    teardown, or module export contract changes.
  - [docs/design/ai.md](../../../docs/design/ai.md) if AI exclusion or encounter behavior needed
    documentation.
- Refresh context capsule section lists only if design-doc structure shifted.
- Update generated or reference surfaces as needed:
  - wiki/stats checks.
  - faction catalog mirror checks.
  - any dev scenario index or lab scenario catalog references.
- Finalize patch-note bullets:
  - City Centre Scout Plane unlocked by Gun Works or Vehicle Works.
  - 50 Steel / 50 Oil, 600-tick build time, 0 supply.
  - one active or in-production plane per player.
  - one Pump Jack worth of Oil upkeep while active.
  - launch to rally point, orbiting, retargeting, and dismiss.
  - 12-tile aerial vision through terrain/building blockers but not smoke.
  - non-combat, non-targetable, non-colliding behavior.
  - any rough visual/audio follow-up.
- Confirm no stray debug logs, debug events, hidden console output, temporary labels, or accidental
  lobby/match messages remain.
- Confirm no accidental AI production/management path exists.
- Do not add new balance tuning, combat, anti-air, audio, or AI behavior in this phase unless the
  user explicitly approves it.

## Expected Touch Points

- `plans/scoutplane/requirements.md`
- `plans/scoutplane/plan.md`
- `plans/scoutplane/phase-*.md`
- `docs/design/protocol.md`
- `docs/design/balance.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `docs/design/ai.md`
- `docs/context/*.md` only if section lists shifted
- `docs/pr-first-workflow.md` only if the phase process itself changed
- generated wiki/stats or catalog artifacts if this repo checks them in

## Edge Cases To Cover

- Requirements and implementation agree on cost, build time, supply, unlock, active limit, launch,
  movement, orbit radius, upkeep, fuel reserve, fog, smoke, projection, commands, visuals, AI
  exclusion, and non-goals.
- Every intentionally deferred item is named: final art, audio, AI usage, tuning watch points, or
  non-goals.
- Docs do not claim the plane can attack, be attacked, block movement, ignore smoke, or be managed
  by AI.
- Review package names exactly how to create, inspect, command, and dismiss the plane.
- Patch notes are factual and do not overstate strategic impact beyond the implemented behavior.
- No old hidden-phase warnings still claim the unit is unavailable after Phase 6+ has exposed it.

## Verification

- `node scripts/check-docs-health.mjs`.
- `node scripts/check-wiki.mjs` if generated stats/wiki surfaces changed.
- `node scripts/check-faction-catalog-parity.mjs` if catalog/config surfaces changed.
- `node tests/select-suites.mjs --from=origin/main` if suite selection docs or mappings changed.
- Focused tests from Phase 7 only if documentation audit finds a missed behavior edge and the fix is
  included in this phase.
- `git diff --check`.

## Manual Test Focus

Use the final review route from Phase 7 and confirm it still matches the docs. The manual pass should
check the City Centre button, unlock, train, launch, orbit, retarget, upkeep, dismiss, enemy
visibility, and rough visual readability once more before final review.

## Handoff Expectations

Name the final patch-note bullets, manual review route, docs touched, generated/reference checks
run, remaining deferred items, and any playtest watch points. The handoff should let a reviewer
understand the finished Scout Plane feature without reading every implementation commit.
