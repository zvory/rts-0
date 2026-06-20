# Phase 4 - PR Lifecycle, Checkpoint, and Daily Runner

Status: done.

## Goal

Finish the sweeper as a PR-first daily gardening workflow. The tool should be able to create or
update one owned docs-drift PR, wait for it to merge, and advance the reviewed checkpoint only
after the merged head is reachable from `origin/main`.

## Scope

- Add an operator command that runs the full sweep:
  - fetch `origin/main`
  - create or reuse an isolated sweep worktree
  - run dry collection
  - run classifier
  - generate docs
  - commit any docs changes
  - push one `zvorygin/` branch
  - open or update an owned PR through `scripts/agent-pr.sh`
  - arm auto-merge
  - wait with `scripts/wait-pr.sh`
  - advance the reviewed checkpoint only after merge reachability is confirmed
- Make no-op sweeps exit successfully without opening a PR or advancing incorrectly.
- Store run reports and generated decisions in an ignored local output directory.
- Add a local `launchd`-friendly wrapper or documented command for a daily 8 p.m. MacBook run.
- Add recovery behavior for failed PR creation, failed checks, closed PRs, stale branches, and
  Codex CLI failures.
- Ensure checkpoint updates are atomic enough that an interrupted run does not skip commits.

## Checkpoint Rules

- The checkpoint tracks one latest reviewed commit, not one tag per source commit.
- The checkpoint must represent `origin/main`, not an unmerged branch head.
- The checkpoint advances after a no-op sweep only when every commit in the scanned range was
  processed and no docs update was needed.
- The checkpoint advances after a docs update only after `scripts/wait-pr.sh <pr>` confirms the PR
  merged and the docs sweep head is reachable from `origin/main`.
- A failed, closed, conflicted, or human-blocked PR leaves the checkpoint unchanged.

## Expected Touch Points

- `scripts/` for the full operator command and scheduling wrapper.
- `tests/` for no-op behavior, checkpoint safety, PR command construction, and failure handling.
- `.gitignore` if a new local report/cache directory is introduced.
- `docs/pr-first-workflow.md` or `docs/context/testing.md` only if the new operator command needs
  to be discoverable by future agents.

## Out of Scope

- Do not install a user's local `launchd` job automatically.
- Do not push directly to `main`.
- Do not bypass branch protection.
- Do not make this daily sweeper a required CI check.
- Do not delete old reports automatically unless a retention policy is explicitly added.

## Verification

Run the focused unit tests plus dry-run lifecycle tests:

```bash
node tests/docdrift_sweeper.mjs
node scripts/docdrift-sweep.mjs --full --dry-run --base <test-sha-or-ref> --head origin/main
git diff --check
```

If safe in the implementation context, run a real no-op or tiny-range PR lifecycle smoke test on a
throwaway branch and record whether `scripts/agent-pr.sh` and `scripts/wait-pr.sh` completed.

## Manual Testing Focus

Run the full command in dry-run mode and inspect the operator output. Confirm that the run makes it
obvious whether a PR was opened, whether auto-merge was armed, whether the checkpoint moved, and
what exact command should be used to recover from a blocked run.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must include the full daily command,
the checkpoint ref or state path, the report/cache paths, exact verification results, any PR link
from lifecycle smoke testing, and clear recovery guidance for the next operator.
