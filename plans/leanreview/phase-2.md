# Phase 2 - Incremental CI-Fix Review

## Phase Status

- [ ] Ready after Phase 1 is merged.

## Objective

Avoid re-reviewing an unchanged full branch after a deterministic CI correction. Preserve the first
full adversarial pass, then use a verified prior reviewed head as the base for one simple linear
follow-up range; anything ambiguous returns to the current full review.

## Review Modes

### Full

Use the existing `origin/main...HEAD` branch review when opening a non-Markdown PR or whenever a
safe incremental anchor cannot be proven. The resulting final head receives the normal
`adversarial-quality-pass` success status and becomes the recorded reviewed head.

### Incremental

Use `reviewedHead..HEAD` only when all of these conditions hold:

- the existing open PR body contains a strictly parsed wrapper-owned reviewed-head SHA;
- GitHub reports a successful `adversarial-quality-pass` status on that exact SHA;
- the SHA exists locally and is a strict ancestor of current `HEAD`;
- every commit after the reviewed head is single-parent, so no merge or rewritten-history case is
  hidden in the range; and
- the range is non-empty and the PR still targets the expected base branch.

The prompt must state that the earlier branch was fully reviewed, name the verified anchor, and ask
for correctness and architecture review of the new range plus its interaction with the already
reviewed result. It must not invite a second pass over unchanged earlier code.

### Already Reviewed

When current `HEAD` exactly equals the verified reviewed head and its success status is still
present, do not launch Codex or repost the status. Preserve the existing durable quality report and
continue only the ordinary PR metadata/auto-merge lifecycle that is still needed.

## Work

- Add a wrapper-owned machine-readable reviewed-head marker to the PR body after a successful full
  or incremental pass. Do not let child-authored JSON choose this SHA.
- On an existing PR, parse the marker strictly, query the commit status for that exact SHA, and
  validate local ancestry/history before choosing a mode.
- Keep the marker useful after an agent manually pushes a CI fix: the existing PR body, not the
  current remote PR head alone, identifies the earlier reviewed anchor.
- Extend the quality-pass invocation with an explicit review mode and review base. Reuse the
  existing bounded changed-path manifest relative to that base rather than inventing a second diff
  collector.
- In incremental mode, keep the complete repository available for context while limiting the
  requested review and changed-path metadata to the correction range.
- After Codex changes, formatting, and Phase 1 final-head preflight succeed, post the success status
  on the actual final head and replace the marker with that final SHA.
- Fall back to Full on missing/malformed marker, missing or unsuccessful status, missing commit,
  non-ancestor history, any merge commit in the range, unexpected base, or GitHub lookup failure.
  Log one concise fallback reason without treating normal first review as an error.
- Keep the current model and reasoning behavior unchanged. This phase must not add model routing,
  effort overrides, or path-risk tiers.
- Record wrapper-owned `Review mode` and `Review base` lines in the durable PR report. Do not append
  an unbounded history of prior reports.
- Update `docs/design/testing.md` and `docs/pr-first-workflow.md`, including the corrected CI-recovery
  instruction: a pushed linear fix may use the prior body marker, while ambiguous history receives
  a full pass.

## Expected Touch Points

- `scripts/agent-pr.sh`
- `scripts/adversarial-quality-pass.mjs`
- `tests/adversarial_quality_pass.mjs`
- `docs/design/testing.md`
- `docs/pr-first-workflow.md`

## Required Tests

- A new non-Markdown PR always uses Full mode.
- A prior marker plus successful status and a simple descendant fix selects Incremental mode with
  the prior SHA as the review base.
- A manually pushed fix is still anchored from the prior PR-body marker rather than mistaken for an
  already reviewed remote head.
- Current `HEAD` equal to the verified reviewed head launches no Codex child and preserves the prior
  report.
- Missing, malformed, failed, or forged marker/status combinations select Full mode.
- A non-ancestor SHA, merge commit, changed base, or GitHub lookup failure selects Full mode.
- Incremental changed-path metadata excludes unchanged earlier branch work and the prompt forbids
  reopening that work while still permitting interaction checks.
- A Codex-modified incremental result records and posts status on the actual final SHA, not the old
  anchor or pre-review head.
- Markdown-only skip, Phase 1 preflights, clean-worktree enforcement, push/status ordering, and
  report rendering remain green.

## Verification

- `node tests/adversarial_quality_pass.mjs`
- `bash -n scripts/agent-pr.sh tests/run-all.sh`
- `node --check scripts/adversarial-quality-pass.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Use a disposable PR fixture or dry-run harness to exercise three cases: first full review, one
linear correction after a recorded successful head, and a merge/rebase-shaped correction. Confirm
the second case presents only the correction range to the child, the third falls back to Full, and
a fourth no-change rerun launches no child.

## Handoff Expectations

Report the marker format, status/ancestry checks, exact Full fallback conditions, no-change behavior,
and how the incremental base reaches the prompt and input manifest. Tell the Phase 3 agent where to
add focused verification context without changing the reviewed-head decision or reopening the full
branch during an incremental pass.

## Deferred

- Incremental review across merge commits, rebases, or rewritten history.
- Searching multiple historical statuses when the recorded anchor is invalid.
- Model selection or reasoning-effort routing.
- Token accounting, budgets, or persisted child sessions.
