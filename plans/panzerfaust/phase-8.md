# Phase 8 - Docs, Patch Notes, And Review Package

## Phase Status

Status: pending.

## Objective

Close out the Panzerfaust feature with final documentation alignment, patch notes, and a reviewable
handoff. This phase should not postpone required contract docs from earlier phases; it exists to
catch drift and package the completed feature clearly.

## Scope

- Review and align product/design docs:
  - [plans/panzerfaust/checklist.md](checklist.md).
  - [docs/design/balance.md](../../docs/design/balance.md).
  - [docs/design/protocol.md](../../docs/design/protocol.md).
  - [docs/design/server-sim.md](../../docs/design/server-sim.md).
  - [docs/design/client-ui.md](../../docs/design/client-ui.md).
  - [docs/design/testing.md](../../docs/design/testing.md) if dev scenarios or test policy changed.
- Verify generated and mirrored data surfaces:
  - Faction catalog exports.
  - Client config mirrors.
  - Wiki/stats output.
  - HUD command-card descriptors and hotkey profiles.
- Finalize patch-note bullets:
  - Barracks gains Panzerfaust after Training Centre.
  - Cost, supply, build time, HP, sight, speed, range, and one-shot damage.
  - Tank-only target filter and conversion to Rifleman.
  - Methamphetamines and Entrenchment interactions.
  - UI, visual, and audio affordances.
  - Known playtest watch points without guessing at strategic impact.
- Record deferred items explicitly:
  - Direct Training Centre production alternative.
  - Broader armored/hard target filters.
  - Hull-facing multipliers.
  - Final art/audio polish beyond the first pass.
  - AI training strategy.
  - Any tuning observations from Phase 7.
- Confirm no debug-only messages, hidden labels, stray logs, temporary event names, unreviewed
  placeholders, or accidental lobby/match messages remain.
- Prepare the final implementation handoff with how to try the unit, what was tested, what changed
  for players, and what remains deferred.

## Expected Touch Points

- `plans/panzerfaust/checklist.md`
- `plans/panzerfaust/plan.md`
- `plans/panzerfaust/phase-*.md`
- `docs/design/balance.md`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `docs/design/testing.md`
- Generated stats/wiki or catalog source files only if checks reveal drift

## Edge Cases To Cover

- Contract docs match the implementation instead of restating the original plan if a previous phase
  made an approved adjustment.
- Patch notes do not promise AI usage, target filters, hull-facing multipliers, reloads, or final
  polish that the feature does not include.
- Deferred items are explicit enough for a later agent to scope without rereading every phase.
- The final review package identifies the manual scenario or tech path a human should use first.

## Verification

- `node scripts/check-docs-health.mjs`.
- `node scripts/check-faction-catalog-parity.mjs`.
- `node scripts/check-wiki.mjs`.
- `node tests/select-suites.mjs --from=origin/main` if the phase changes enough files that suite
  selection should be checked.
- `git diff --check`.

## Manual Test Focus

No new manual gameplay test is required if Phase 7 already completed it. The final review package
should still name the best smoke path: train a Panzerfaust from Barracks after Training Centre, fire
once at a Tank, observe conversion to Rifleman, and inspect one fog/replay case.

## Handoff Expectations

Provide the final player-facing summary, exact verification run, manual test notes, known deferred
items, and patch-note bullets. Confirm every Panzerfaust phase document that has been implemented is
marked done by its implementation PR and that no future phase is reported complete before its PR is
merged.
