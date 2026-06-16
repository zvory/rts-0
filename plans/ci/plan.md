# PR-first CI workflow plan

## Status

Draft.

## Summary

Move the repository from local full-suite commit gates and direct `main` merges to a PR-first
workflow where GitHub Actions is the authoritative full gate. Agents should still use isolated
worktrees and focused local verification, but completed implementation work should end as an owned
PR with auto-merge armed, or as a clear blocked handoff with a PR link and failing evidence. Serial
phase execution must not continue until the previous phase PR is definitely merged and its head is
reachable from `origin/main`.

## Cross-phase constraints

- Keep the change reversible: the repository should not depend on paid or larger GitHub-hosted
  runners, and CI should remain runnable through `tests/run-all.sh` locally or on another runner.
- Treat public-repository GitHub Actions minutes as a useful current property, not a permanent
  architectural guarantee. GitHub currently documents standard GitHub-hosted runner usage as free
  for public repositories, while larger runners and storage/cache limits still matter.
- Use standard `ubuntu-latest` runners unless a later phase explicitly documents a paid-runner
  decision.
- Keep PR ownership machine-checkable for agent branches. A normal `zvorygin/*` PR should name its
  owner, lifecycle mode, auto-merge state, and focused verification in a predictable body format or
  labels.
- No agent should start the next serial phase from an assumed merge. Serial automation must verify
  the PR is merged through GitHub and verify the phase head is an ancestor of `origin/main`.
- Avoid merge-queue-only designs. This repo is a personal public repository today, so the durable
  path is branch protection, required checks, auto-merge, stale-branch handling, and explicit PR
  sweeps. The initial protection uses admin bypass for emergency repair and this migration's
  remaining direct-merge phases only.
- Keep broad local test runs optional for agents. Focused local verification remains required when
  it is useful for fast feedback, but the full gate moves to PR CI.
- Every phase handoff should name the next implementation step and the core manual checks to run.

## Phase summaries

1. Phase 1 defines the exact GitHub-side gate and updates Actions so the full required signal runs
   on PRs before branch protection depends on it. It also documents the CI cost posture, required
   check names, and non-goals around merge queue.
2. Phase 2 enables repository settings and branch protection/rulesets for PR-only `main` updates.
   It turns on auto-merge and delete-branch-on-merge, blocks normal direct pushes, and records the
   configured required checks.
3. Phase 3 rewrites local hooks and agent-facing docs so agents stop using the laptop as the full
   gate. It preserves focused verification and makes the PR lifecycle the normal completion path.
4. Phase 4 adds ownership and waiting helpers so PRs do not go unowned under normal operations.
   The helpers should open or audit agent PRs, arm auto-merge, wait cheaply for merge/failure when
   requested, and summarize stale or failed owned PRs.
5. Phase 5 converts `scripts/phase-runner.sh` to PR-first serial execution. It must support
   non-blocking "open and arm PR" mode and blocking "wait until merged before next phase" mode.
6. Phase 6 hardens cleanup, recovery, and rollout validation. It updates worktree cleanup for
   auto-deleted remote branches, adds canary documentation, and verifies the new contract through a
   small docs-only PR and one real implementation PR before declaring the workflow complete.

## Implementation order

Implement one phase at a time. In the current pre-migration repository, follow the active repo
workflow for landing these planning and migration changes. Once Phase 2 and Phase 3 are complete,
later phases should use the new PR-first workflow they introduce.

After each phase, the implementing agent must provide a handoff message that includes:

- the PR or commit that landed the phase;
- focused local verification that was run;
- GitHub CI or settings evidence, when applicable;
- any manual checks needed before the next phase;
- whether the next phase can proceed immediately or must wait for a PR merge.

## Cost and dependency posture

The workflow should use GitHub Actions aggressively enough to protect `main`, but not carelessly.
The plan should avoid expensive runner classes, keep timeouts bounded, add concurrency cancellation
for superseded PR runs where safe, and keep the local `tests/run-all.sh` path healthy as a fallback.
If the repository becomes private, moves to paid runners, or GitHub changes public-repository
runner policy, the workflow should degrade by reducing required PR gates or moving the same scripts
to another runner rather than rewriting the development process.

References for implementation:

- GitHub Actions billing docs currently state that standard GitHub-hosted runners are free for
  public repositories, while private repositories use plan quotas and overage billing.
- GitHub Actions limits docs still apply to public repositories; workflows can be canceled or rate
  limited when platform limits are reached.
- Larger runners are out of scope for this rollout because GitHub bills them differently even when
  public-repository standard runners are free.
