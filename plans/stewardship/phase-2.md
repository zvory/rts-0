# Phase 2 - Make Documentation Stewardship Recoverable

Status: Incomplete.

## Objective

Restore trust in the scheduled documentation stewardship flow. Make stale runner state recoverable,
preserve generated sweep work, and make the route map truthful for the known split source families
without attempting to map every file in the repository.

## Work

- Separate recovery policy for the disposable detached runner from policy for the sweep branch that
  may contain generated documentation commits.
- For a clean stale runner worktree, safely recreate it or detach it at current `origin/main`; the
  runner owns no generated documentation that needs preservation.
- For a clean diverged sweep branch, preserve its commits and existing PR/run identity. Resume that
  work when safe or start a clearly recorded new run branch from current `origin/main`; never reset
  or discard generated commits merely to unblock the schedule.
- For dirty worktrees or merge/rebase conflicts, fail closed with concise state and exact recovery
  instructions. Do not automatically stash, reset, or delete work.
- Refresh `docs/doc-map.json` only for the reviewed split config, protocol, replay, match-history,
  and deployment source families. Do not broaden this into a comprehensive repository mapping pass.
- Make docs health reject a configured source route that matches no tracked files, and add focused
  coverage for representative valid and stale routes.
- Cover clean stale-runner recovery, clean diverged-sweep preservation, and dirty/conflicted
  fail-closed behavior in focused tests.

## Non-goals

- Do not rewrite the doc-drift classifier, generation prompts, PR lifecycle, or checkpoint model.
- Do not auto-resolve documentation merge conflicts or perform destructive Git recovery.
- Do not map every presently unmapped source file.

## Expected Touch Points

- `scripts/docdrift-daily.sh`
- `scripts/docdrift-sweep.mjs`
- `tests/docdrift_sweeper.mjs`
- `docs/doc-map.json`
- `scripts/check-docs-health.mjs` and focused docs-health tests
- documentation for the doc-drift operator flow only where recovery behavior changes

## Verification

- `node tests/docdrift_sweeper.mjs`
- `node scripts/check-docs-health.mjs`
- Run the doc-drift flow in safe preview/dry-run mode against a realistic stale runner and confirm it
  reaches current `origin/main` without opening a PR.
- Exercise a fixture with a diverged sweep branch containing a generated commit and prove the commit
  remains reachable after recovery handling.
- Exercise a dirty fixture and confirm the flow stops without modifying its worktree.
- `git diff --check`

## Manual Test Focus

No gameplay test is expected. Read the preview lifecycle once and confirm it clearly distinguishes
runner refresh, preserved sweep work, and a human-required conflict.

## Handoff

Mark this phase done in its implementation commit. Report each recovery case, preserved-commit
evidence, the exact source families added to the route map, and focused verification. Tell the Phase
3 agent that source-size policy remains unchanged.
