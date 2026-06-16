# PR-first workflow recovery

The normal agent lifecycle is:

1. Work on one `zvorygin/*` branch in one `/tmp/rts-worktrees/*` worktree.
2. Run focused local verification for the files or contracts changed.
3. Commit with the normal hook.
4. Open or update the owned PR with `scripts/agent-pr.sh --verification "..."`
   and leave auto-merge armed.
5. Run `scripts/wait-pr.sh <pr>` and do not claim completion until it reports the PR merged and the
   head SHA reachable from `origin/main`.

GitHub Actions owns the full-suite merge gate through the aggregate `./tests/run-all.sh` check in
the `Main test gate` workflow. The workflow runs split coverage jobs for server build,
Rust/architecture, live Node, and browser/tri-state suites, then fails the aggregate check if any
required coverage job fails. The Rust/architecture job installs `cargo-nextest` and runs
`./tests/run-all.sh --only-rust`, matching the local nextest-backed Rust command path. Local hooks
are intentionally cheap; they catch staged whitespace errors and run opportunistic cleanup on
`main`.

## Recovery states

- CI failed: inspect the failing check from the PR or `gh pr checks <pr>`.
  Fix the branch in its worktree, run the smallest relevant local verification,
  commit, push, and rerun `scripts/agent-pr.sh --verification "..."` so the PR
  body records the current evidence.
- Branch stale or conflicted: fetch `origin/main`, merge it into the PR branch,
  resolve conflicts in the same worktree, rerun focused verification, and push.
  Do not claim completion or start follow-up work until `scripts/wait-pr.sh <pr>`
  reports the PR merged and its head reachable from `origin/main`.
- Auto-merge not armed: rerun `scripts/agent-pr.sh --verification "..."` from
  the branch, or run `gh pr merge <pr> --auto --merge` after confirming the PR
  is still agent-owned and should merge when green.
- PR closed unmerged: do not let cleanup remove the worktree unless the head is
  reachable from `main` or `origin/main`. Reopen the PR if the branch is still
  valid, or create a replacement branch and PR from the useful commits.
- GitHub API unavailable: leave the branch and worktree intact. Retry the
  helper later; serial automation must stop because it cannot prove merge state.
- Emergency admin/direct push: use only when explicitly authorized. Record the
  reason in the handoff or commit body, run the verification appropriate for the
  risk, push to `main`, fetch `origin/main` in active checkouts, then preview
  cleanup with `scripts/cleanup-worktrees.sh --dry-run`.

Run `scripts/pr-sweep.sh` when ownership or status is unclear. It lists open
agent PRs and flags missing owner metadata, missing auto-merge, failed CI,
conflicts, stale PRs, and `needs-human` state.

An open PR with auto-merge armed is a pending handoff, not a completed task. If
CI fails, GitHub is unavailable, the PR needs human input, or waiting would be
inappropriate for an explicitly user-requested stop, report the PR link and the
exact blocker instead of calling the work complete.

## Worktree cleanup

`scripts/cleanup-worktrees.sh` removes only clean `zvorygin/*` worktrees whose
branch head is reachable from local `main` or `origin/main`. It does not require
the matching remote branch to exist, so it tolerates GitHub delete-branch-on-merge
after a PR auto-merges. It keeps dirty worktrees and clean worktrees with
unmerged heads.

Use these commands to audit or run cleanup:

```bash
scripts/cleanup-worktrees.sh --dry-run
scripts/cleanup-worktrees.sh
```

The installed hooks run `scripts/cleanup-worktrees.sh --auto` after commits and
merges on local `main`. Auto mode skips cleanup from other branches and removes
only a bounded number of stale Cargo target directories per run.

## Rollout canaries

Before relying on a changed workflow broadly, run three canaries:

- A docs-only branch that opens with `scripts/agent-pr.sh`, has auto-merge
  armed, passes the aggregate `./tests/run-all.sh` check, and merges.
- A representative implementation branch with focused local verification in the
  PR body, auto-merge armed, and a successful merge through the same required
  gate.
- A throwaway docs-only phased plan through
  `scripts/phase-runner.sh --plan <name> --from <start> --to <end> --pr --wait`
  to prove the runner waits for each PR to merge before continuing.

After canaries, run `scripts/pr-sweep.sh` and confirm there are no unowned open
`zvorygin/*` PRs and no unexpected stale, failed, conflicted, or missing
auto-merge states.

## Alternate runner fallback

Keep `tests/run-all.sh` as the portable full gate. If GitHub Actions pricing,
public-repository terms, or runner availability stop fitting the project, move
the same command to another runner instead of changing the agent lifecycle.

The durable contract is:

- PRs target `main`.
- The required check remains a stable full-gate signal named `./tests/run-all.sh`.
- Auto-merge is armed only after ownership metadata and focused verification are
  recorded.
- Serial automation waits for a definite merge and verifies the phase head is
  reachable from `origin/main`.

When moving runners, update the workflow or external CI integration first, then
update branch protection/rulesets to require the replacement check name. Keep
standard GitHub-hosted `ubuntu-latest` runners as the default until a deliberate
paid-runner or alternate-runner decision is recorded.
