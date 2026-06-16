# Phase 4 - PR ownership and wait helpers

## Status

Pending.

## Goal

Add durable tooling so owned PRs are visible, auto-merge is armed consistently, CI failures are
actionable, and agents can either stop cheaply or wait without burning model tokens.

## Scope

- Add a helper for opening or updating an agent PR with a predictable body.
- Add labels or another machine-readable convention for `agent-owned`, `automerge`, `ci-failed`,
  and `needs-human`.
- Add `scripts/wait-pr.sh <pr>` or equivalent:
  - exit `0` only when GitHub reports the PR merged;
  - verify the PR head SHA is reachable from `origin/main`;
  - exit non-zero when required checks fail, cancel, or the PR closes unmerged;
  - sleep between polls without producing noisy logs;
  - summarize failing checks with links.
- Add `scripts/pr-sweep.sh` or equivalent:
  - list open agent-owned PRs;
  - show owner, age, head SHA, auto-merge state, and check state;
  - flag PRs without auto-merge armed;
  - flag stale, conflicted, failed, or human-blocked PRs.
- Add a required or advisory ownership check for `zvorygin/*` PRs if GitHub settings alone cannot
  enforce ownership metadata.

## Expected touch points

- `scripts/`
- `.github/workflows/` if adding an ownership check
- `.github/pull_request_template.md` if useful
- `AGENTS.md`
- `CLAUDE.md`
- `README.md`
- `docs/ci/plan.md`

## Verification

- Run helper scripts against a real or test PR and capture expected JSON/text output.
- Confirm `wait-pr` returns success only after the PR is merged and the head SHA is on
  `origin/main`.
- Confirm `wait-pr` returns failure for a closed-unmerged PR and for a failing required check.
- Confirm `pr-sweep` identifies at least these states in fixtures or mocked JSON: clean pending,
  auto-merge armed, failed CI, missing owner metadata, and needs-human.

## Manual testing focus

Use the sweep output to answer "what PRs are owned by agents, who owns them, and what happens
next?" without opening every PR in a browser.

## Handoff expectations

Document exact command examples for normal agents and serial phase agents. Note any GitHub API
limitations that Phase 5 must work around.

