# Phase 5 - Cleanup Closeout

Status: planned.

## Goal

Close the balance cleanup plan with a no-drift review, documentation refresh, and hotspot tracking
confirmation.

## Scope

- Update `docs/design/balance.md`, `docs/context/balance.md`, client architecture docs, and hotspot
  docs for any internal module paths created by Phases 3 and 4.
- Rerun hotspot analysis and confirm split files still roll up to the `balance-and-config` logical
  group.
- Capture before/after structured balance outputs from the current branch and confirm no stat,
  catalog, ability, upgrade, resource, body, command-budget, public config export, or wiki-visible
  values changed.
- Remove only stale comments or temporary helper code created by this plan.
- Do not move additional balance, config, faction, or wiki logic unless an earlier phase explicitly
  deferred a tiny mechanical cleanup.

## Touch Points

- `docs/design/balance.md`
- `docs/context/balance.md`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`
- `docs/hotspot-analysis.md`
- `scripts/hotspot-analysis.mjs`, only if final grouping needs adjustment
- any temporary scripts or test helpers introduced by earlier phases

## Constraints

- Preserve all gameplay values and public imports.
- Do not collapse the Rust/client ownership boundary after the split. The closeout should clarify
  the boundary, not redesign it.
- Treat any before/after structured output drift as a blocker unless it is an explicitly documented
  docs-only wording change with no runtime data change.
- Do not remove compatibility re-exports or export aliases created by earlier phases unless their
  phase handoff explicitly identified them as temporary and already proved downstream callers no
  longer need them.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-wiki.mjs`
- `node tests/client_contracts.mjs`
- any command-budget/export-name parity command introduced by Phase 2
- `node scripts/check-docs-health.mjs`
- `node scripts/hotspot-analysis.mjs --base-ref HEAD --recent-days 14 --limit 0 --output /tmp/rts-hotspots-after.json`
- `git diff --check`

## Manual Testing Focus

No new gameplay behavior is expected. Manually review the final module map and run one smoke pass
later through lobby start, build menu, unit training, research, and one ability command to confirm
the user-facing config still behaves normally.

## Handoff

Mark this phase done only after committing the closeout. Summarize no-drift evidence, hotspot group
tracking status, docs updated, verification passed, and any remaining balance/config risks that
need a separate future plan.
