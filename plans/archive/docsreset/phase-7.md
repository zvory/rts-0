# Phase 7 - AI, Testing, Dev Scenarios, and Tooling

Status: Done.

## Goal

Fix stale active docs for AI behavior, self-play, test commands, dev scenarios, and repo tooling.

## Scope

- Audit AI profile names, aliases, default AI behavior, opening/economy/tech descriptions,
  defensive panic behavior, self-play scenario expectations, dev scenario names, test command docs,
  CI/local hook docs, and docs-health/wiki tooling references.
- Fix stale docs that conflict with current AI code, test scripts, or tooling behavior.
- Update capsules only when their routing pointers or commands are stale.
- Do not retune AI, rewrite tests, or change CI behavior unless a deterministic docs check is
  wrong and the minimal fix is in scope.

## Suggested Evidence

- `docs/context/server-sim.md`
- `docs/context/testing.md`
- `docs/design/ai.md`
- `docs/design/testing.md`
- `docs/pr-first-workflow.md`
- `server/crates/ai/src/**`
- `server/src/bin/ai_*.rs`
- `server/src/tools/ai_*.rs`
- `scripts/ai-*.sh`
- `scripts/check-*.mjs`
- `tests/README.md`
- `tests/ai_integration.mjs`
- `tests/run-all.sh`

Useful searches:

```bash
rg -n "AI|ai_|profile|self-play|scenario|dev scenario|test|nextest|run-all|hook|CI|docs-health|wiki" docs/design docs/context docs/pr-first-workflow.md tests scripts server/crates/ai -S
```

## Verification

Run focused checks that match the final diff. Likely commands:

```bash
node scripts/check-docs-health.mjs
git diff --check
```

If AI docs are materially changed and a narrow AI command is cheap and relevant, run the specific
test/tool command identified by the executor. Do not run broad self-play by default.

## Manual Testing Focus

Later manual/test focus should cover any AI profile, self-play scenario, or documented command that
the phase changed.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must list AI/testing/tooling claims
fixed, source evidence checked, verification run, and any expensive validation intentionally left to
CI or later manual review.
