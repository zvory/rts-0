# PR-first workflow recovery

The normal agent lifecycle is:

1. Work on one `zvorygin/*` branch in one `/tmp/rts-worktrees/*` worktree.
2. Run focused local verification for the files or contracts changed.
3. Commit with the normal hook.
4. Open or update the owned PR with `scripts/agent-pr.sh --verification "..."`.
   Before the quality pass, the helper archives any plan newly completed by the branch and commits
   that move, so the archive lands in the final phase PR rather than as a post-merge local change.
   It then runs the ordered specialist passes in `scripts/agent-pr-passes.json`. The patch-note pass
   uses a bounded branch diff and creates or refreshes one dated, branch-keyed fragment only for a
   change to an active participant's experience in an ordinary live match. Spectator/observer,
   replay, match-history, Lab/dev, lobby/setup, and analysis-only changes are explicitly excluded,
   even when they are user-facing. Each configured pass can select its own Codex model through its
   `modelEnv` setting. The final adversarial pass runs after all specialist edits and verifies any
   generated patch note against the final diff.
   The helper first classifies the branch diff against `origin/main`. If every changed file ends in
   `.md`, it skips Codex adversarial review, pushes the branch, posts a successful
   `adversarial-quality-pass` status with a docs-only skip description, and writes the skip report
   into the PR body. Otherwise it runs `scripts/adversarial-quality-pass.mjs` in the branch worktree,
   allowing a fresh Codex CLI pass to improve or rewrite the branch, commit the final state, push
   the final head, post the `adversarial-quality-pass` status, and write the full quality-pass
   report into the PR body.
5. Run `scripts/wait-pr.sh <pr>` and do not claim completion until it reports the PR merged, the
   head SHA reachable from `origin/main`, and the local `main` checkout fast-forwarded with an
   ordinary `git pull --ff-only origin main`. The final refresh also runs the existing automatic
   merged-worktree cleanup, including when `main` was already current and Git's `post-merge` hook
   therefore did not fire. After proving merge reachability and before cleanup, the waiter delivers
   any patch-note fragment changed by that PR to Discord. Rerunning `scripts/agent-pr.sh` during CI
   recovery only regenerates the fragment; it does not notify Discord.

GitHub Actions owns the full-suite merge gate through the aggregate `./tests/run-all.sh` check in
the `Main test gate` workflow. The workflow runs split coverage jobs for server build, Rust
policy/lint, two complementary Rust nextest partitions, live Node, and two browser/tri-state shards,
then fails the aggregate check if any required coverage job fails. The nextest shards install
`cargo-nextest` and run `./tests/run-all.sh --only-nextest` with complementary `slice:1/2` and
`slice:2/2` partitions, matching the local nextest-backed Rust command path without dropping tests.
Local hooks
are intentionally cheap; they catch staged whitespace errors outside the human-owned
`playtest_notes.md`, run `node scripts/check-docs-health.mjs`, and run opportunistic cleanup on
`main`. Branch protection requires the `adversarial-quality-pass` status alongside
`./tests/run-all.sh`; this status means either the autonomous quality pass ran on the final PR head
or the PR helper classified the branch as pure Markdown (`.md`) and intentionally skipped it. It is
not a substitute for the full test gate. The owned PR body keeps the pass or skip summary, issues
found, changes made, verification, and remaining concerns for post-merge audit.

When the Rust job is slow, use the ordinary job log first: the Rust context lines show
`CARGO_TARGET_DIR`, Rust/cargo/nextest versions, and the Actions Cargo cache exact-hit result,
while the `tests/run-all.sh` timing summary and nextest output show whether time was spent
compiling or running specific tests. Do not add a separate diagnostic workflow for that question.

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
  the branch so the quality pass and status are refreshed, or run
  `gh pr merge <pr> --auto --merge` after confirming the PR is still agent-owned
  and should merge when green.
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
only a bounded number of stale Cargo target directories per run. `scripts/wait-pr.sh`
also invokes auto cleanup explicitly after refreshing the local `main` checkout so
post-PR cleanup does not depend on whether the pull advanced `main` enough to fire
the `post-merge` hook.

## Rollout canaries

Before relying on a changed workflow broadly, run three canaries:

- A pure Markdown branch that opens with `scripts/agent-pr.sh`, skips Codex adversarial review,
  posts the `adversarial-quality-pass` status, has auto-merge armed, passes the aggregate
  `./tests/run-all.sh` check, and merges.
- A representative implementation branch with focused local verification in the
  PR body, auto-merge armed, and a successful merge through the same required
  gate.
- A throwaway docs-only phased plan through
  `scripts/phase-runner.sh --plan <name> --from <start> --to <end> --pr --wait`
  to prove the maintained Node runner waits for each PR to merge before continuing.

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
- The required status `adversarial-quality-pass` records that the final autonomous quality pass ran
  on the PR head, except for pure Markdown PRs where it records the intentional docs-only skip.
- The PR body records the full quality-pass report, or the docs-only skip report, including
  remaining concerns.
- Auto-merge is armed only after ownership metadata and focused verification are
  recorded.
- Serial automation waits for a definite merge and verifies the phase head is
  reachable from `origin/main`.

When moving runners, update the workflow or external CI integration first, then
update branch protection/rulesets to require the replacement check name. Keep
standard GitHub-hosted `ubuntu-latest` runners as the default until a deliberate
paid-runner or alternate-runner decision is recorded.
