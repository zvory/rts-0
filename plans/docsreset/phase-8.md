# Phase 8 - Final Consistency Sweep

Status: done.

## Goal

Run a final active-doc consistency pass after the sector audits have merged, catching remaining
obvious stale terms and contradictions without rewriting broad prose.

## Scope

- Search active docs and generated-doc code for retired terms, contradiction clusters, stale routing
  pointers, and references to files or commands that no longer exist.
- Re-run focused docs/wiki validation and patch only confirmed remaining drift.
- Update context capsules only when their pointers, section names, or command references are stale.
- Leave unresolved or ambiguous claims in the handoff rather than forcing speculative edits.
- Do not perform broad reorganization, style cleanup, or product redesign.

## Suggested Evidence

- `docs/context/README.md`
- all `docs/context/*.md`
- all `docs/design/*.md`
- `docs/doc-map.json`
- `server/src/wiki.rs`
- `scripts/check-docs-health.mjs`
- `scripts/check-wiki.mjs`
- `scripts/check-faction-catalog-parity.mjs`
- current `fd` file lists for referenced paths

Useful searches:

```bash
fd -t f '.*' docs/context docs/design
rg -n "Steelworks|TODO|outdated|future|placeholder|old|deprecated|does not exist|no longer|FIXME" docs/context docs/design server/src/wiki.rs scripts -S
rg -n "\]\(([^)#]+)\)" docs/context docs/design -S
```

## Verification

Run focused checks:

```bash
node scripts/check-docs-health.mjs
node scripts/check-wiki.mjs
node scripts/check-faction-catalog-parity.mjs
git diff --check
```

Add any narrow check required by files changed in this phase.

## Manual Testing Focus

After this phase merges, manually spot-check `/wiki`, `/wiki/stats`, and the design docs most
heavily changed by the reset.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must summarize the final stale-term
searches, verification run, remaining unresolved claims, and any recommended follow-up for the
recurring docs drift sweeper.
