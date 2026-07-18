# Phase 2 - Make Documentation Stewardship Recoverable

Status: Incomplete.

## Objective

Make the scheduled documentation sweep recover deterministically from the fixed sweep branch's
real lifecycle states. Preserve generated commits and old run refs, resume work only when safe, and
start completed runs from a fresh unique branch without changing documentation routing or the
classifier/generator model.

## Work

- Resolve the sweep branch, its worktree, local and remote heads, recorded run identity, and any
  associated owned PR before changing a ref, worktree, checkpoint, or GitHub state.
- Persist one atomically updated run-state record under `.docdrift/runs/<run-id>/` before branch
  creation and after every lifecycle step. The record is the recovery authority and must contain a
  schema version, run id, base/head SHAs, branch, worktree, generated head SHA, lifecycle step
  statuses, PR number/URL/state when known, checkpoint target, and last update time. Recovery may use
  an explicit `--run-id` or the single recorded nonterminal run; multiple candidates, multiple PR
  matches, or Git/ref state that disagrees with the record fail closed.
- Add one explicit bootstrap for the pre-record fixed branch that caused this plan. Adopt
  `zvorygin/docdrift-sweep` as a terminal legacy run only when its local, remote, and clean worktree
  heads are identical and exactly one owned PR has that exact head. Record the PR's terminal state
  and base/head evidence atomically before applying the normal table below; never resume or rewrite
  the legacy head. The current regression fixture is the clean `68f6e958...` head with closed,
  unmerged PR #627. Its terminal GitHub mergeability is currently `DIRTY`; that stale conflict
  classification does not block this exact closed-unmerged legacy adoption because the PR can no
  longer merge and its preserved head is verified directly. Any different or ambiguous pre-record
  state fails closed with an explicit operator-adoption command rather than guessing.
- Apply one deterministic policy based on PR/run state:
  - **Open PR:** preserve and reuse the same branch, commits, PR, and run identity. Resume from the
    first incomplete recorded lifecycle step rather than regenerating or opening another PR. The
    only automatic ref reconciliation allowed is a fast-forward of a clean local branch to the
    matching recorded remote head. If the worktree is absent, recreate it from a local or remote
    branch only when that branch head exactly matches the recorded generated head; otherwise fail
    closed. A conflicted PR, non-fast-forward local/remote heads, or a head mismatch also fails.
  - **Merged PR:** preserve the completed branch/ref and its report. Finish any idempotent
    post-merge checkpoint step supported by recorded evidence, then allocate a unique run branch and
    worktree from the current fetched `origin/main` for later unprocessed commits.
  - **Closed, unmerged PR:** preserve the branch, commits, PR identity, and unchanged checkpoint.
    Treat mergeability/conflict status as terminal metadata rather than a resumability decision;
    exact head and identity agreement still apply. Allocate a unique new run branch and worktree
    from current `origin/main`; the normal checkpoint range may regenerate the abandoned
    documentation work without rewriting the old ref.
  - **No PR yet:** a clean branch tied to a recorded in-progress run may continue to its first
    incomplete push/PR step. A missing worktree may be recreated only from the exact recorded local
    or remote branch head. If generated commits cannot be tied unambiguously to one recorded run,
    fail closed and print the exact refs and report paths requiring operator review.
- Derive new run branch and worktree names from the run id under the `zvorygin/` namespace. Verify
  that neither local nor remote refs nor worktree paths already exist; never reuse, reset, delete,
  or force-update an old run merely to obtain the preferred name.
- Treat dirty worktrees, unmerged index entries, merge/rebase/cherry-pick state, conflicting
  local/remote heads, an open PR reported as conflicted, and any ambiguous PR match as hard stops.
  A closed-unmerged PR's stale mergeability classification follows the terminal rule above and is
  not itself a hard stop. Report the exact branch, SHAs, PR state, worktree status, and safe operator
  choices. Do not automatically stash, reset, rebase, force-push, delete, or resolve conflicts.
- Keep recovery decisions visible in the full-sweep lifecycle/report so a scheduled failure says
  whether it resumed an open run, completed a merged run, created a fresh run branch, or stopped for
  human review.
- Add behavior tests using temporary Git repositories with a bare `origin`, real branches,
  worktrees, and commits. Stub only GitHub PR-state responses and owned-PR/wait boundaries; do not
  replace Git behavior with source-text or command-string assertions.

## Non-goals

- Do not change `docs/doc-map.json`, source-route health, the doc-drift classifier, generation
  prompts, patch semantics, PR ownership, or checkpoint advancement policy beyond idempotently
  completing a verified merged run.
- Do not broaden this phase into detached daily-runner cleanup; the runner remains disposable and
  already refreshes independently from the generated sweep branch.
- Do not auto-resolve conflicts or perform destructive Git recovery.
- Do not collapse all runs back onto `zvorygin/docdrift-sweep` after a PR reaches a terminal state.

## Likely Touch Points

- `scripts/docdrift-sweep.mjs`
- `tests/docdrift_sweeper.mjs`
- one schema-versioned run-state JSON record written beneath the existing per-run report directory
- a small test seam for PR-state lookup, only if needed for deterministic stubbing
- `docs/design/testing.md` where the full-sweep recovery contract is documented

## Verification

- `node tests/docdrift_sweeper.mjs`
- In temporary repositories with a bare origin and stubbed PR states, prove:
  - a clean open-PR run resumes the same branch/PR without creating or rewriting a ref;
  - merged and closed-unmerged terminal PRs leave the old branch SHA reachable and create a
    distinct run-id branch at the exact fetched `origin/main` head;
  - a clean recorded pre-PR run preserves its generated commit and continues to the PR boundary;
  - local-only and remote-only recorded branches recreate a missing worktree only when their exact
    heads match the run-state record;
  - the pre-record fixed-branch fixture with matching clean local/remote/worktree head and one
    closed-unmerged `DIRTY` PR is adopted, preserved, and followed by a unique new run branch, while
    any second matching PR or head mismatch fails closed;
  - an open conflicted PR fails closed, while a recorded closed-unmerged PR with exact identity and
    head agreement proceeds regardless of terminal mergeability metadata;
  - dirty and conflicted worktrees exit non-zero before refs, checkpoints, or stubbed GitHub state
    change; and
  - ambiguous run records, multiple matching PRs, head mismatches, and branch-name/ref collisions
    fail closed without overwriting either run.
- Run a full lifecycle preview and confirm its report names the selected recovery action and branch.
- `git diff --check`

## Manual Test Focus

No gameplay test is expected. Read one report for each open, terminal, and fail-closed path and
confirm it identifies preserved refs and the exact next action without suggesting destructive
recovery.

## Handoff

Mark this phase done in its implementation commit. Report the PR-state table, old-ref reachability
evidence, unique new branch naming, and dirty/conflict failure evidence. Tell the next agent that
documentation routes and source-size policy were not changed by this phase.
