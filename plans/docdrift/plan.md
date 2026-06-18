# Documentation Drift Sweeper Plan

## Purpose

Build a scheduled documentation gardening tool that reviews behavior-changing commits since the
last reviewed checkpoint and updates authoritative design docs when the docs appear stale. The tool
is intentionally probabilistic: it should make regular doc drift cleanup cheap and routine, not
prove that every semantic change is perfectly documented. The implementation assumes a separate doc
audit has already produced a usable trace map from repo surfaces to likely authoritative docs.

## Prerequisites

- A doc audit has confirmed that `docs/design/*.md` remains small enough and structured enough for
  targeted model reads.
- A committed trace map exists and lists likely authoritative docs for broad repo surfaces. The map
  may be many-to-many and imperfect; it is a routing hint, not a proof obligation.
- A single reviewed checkpoint ref or tag exists, representing the latest `origin/main` commit that
  the sweeper has fully processed.
- The OpenAI model invocation path is decided before implementation begins. Model names,
  credentials, rate limits, and local environment variables must stay configurable rather than
  hard-coded.

## Operating Model

- The sweeper compares the checkpoint to `origin/main`.
- For each non-merge commit, it reads the commit subject/body, changed paths, compact diff stats,
  and whether likely docs changed.
- The cheap classification pass chooses only between "move on" and "update docs." It may include
  a short evidence note and likely doc targets, but it does not block on a manual-review category.
- The first implementation should not read full diffs by default. If later evidence shows commit
  metadata is too weak, a bounded targeted-hunk mode can be added inside the same cheap classifier
  pass and under an explicit token budget.
- The stronger update pass edits relevant authoritative docs with factual, minimal changes that a
  future agent can rely on.
- All generated doc changes land on one `zvorygin/` branch for the sweep. The implementation may
  choose one aggregate commit or one commit per source commit, as long as the branch remains a
  single owned PR.
- The checkpoint advances only after the docs PR is merged and the merged head is reachable from
  `origin/main`.

## Non-Goals

- Do not make this a required blocking CI gate in the first implementation.
- Do not require perfect recall, perfect code-to-doc mapping, or manual triage of uncertain commits.
- Do not tag every source commit. Track only the latest reviewed checkpoint.
- Do not let generated docs rewrite broad prose, reorganize design docs, or update capsules unless
  the authoritative design doc structure actually changed.
- Do not read or print model credentials, Fly tokens, or other local secrets.

## Phase Summaries

### [Phase 1 - Sweep Contract and Dry Run](phase-1.md)

Create the local sweep command, checkpoint reader, trace-map loader, commit metadata collector, and
dry-run report format. This phase should make it possible to run from the last reviewed checkpoint
to `origin/main` and see which commits would be considered, which docs the trace map routes them
toward, and which commits are skipped because they are merge commits or docs-only churn. It should
not call a model or edit docs yet; the value is a deterministic, cheap operator surface.

### [Phase 2 - Cheap Commit Classifier](phase-2.md)

Add the model-backed classification pass that decides, per commit, whether to move on or request a
doc update. The classifier should receive only commit metadata, changed paths, compact diff stats,
docs touched, and trace-map candidates, then emit a small structured decision record that can be
cached and replayed. It should avoid manual-review states; ambiguous cases should be resolved by
the classifier's best judgment and recorded as either move-on or update-docs.

### [Phase 3 - Doc Patch Generator](phase-3.md)

Add the stronger model pass that turns update-docs decisions into minimal edits to authoritative
design docs. This phase should feed the model only the selected decision records, relevant design
doc sections, and compact commit evidence, then apply patches into one sweep branch. It is done
when a local dry run can generate a clean docs diff, rerun idempotently, and pass focused docs
verification.

### [Phase 4 - PR Lifecycle, Checkpoint, and Daily Runner](phase-4.md)

Wire the sweeper into the repo's PR-first lifecycle and add local scheduling support. This phase
should create or update one owned PR for a sweep, arm auto-merge, wait for the merge, and advance
the reviewed checkpoint only after the merged docs head is reachable from `origin/main`. It should
also provide a local `launchd`-friendly command or wrapper for the user's MacBook daily run without
making the scheduled job mandatory for other developers.

## Phase Index

1. [Phase 1 - Sweep Contract and Dry Run](phase-1.md)
2. [Phase 2 - Cheap Commit Classifier](phase-2.md)
3. [Phase 3 - Doc Patch Generator](phase-3.md)
4. [Phase 4 - PR Lifecycle, Checkpoint, and Daily Runner](phase-4.md)

## Overall Constraints

- Preserve the repo's existing source-of-truth rule: `docs/design/*.md` are authoritative, while
  `docs/context/*.md` are pointers. Update capsules only when section lists or entry points change.
- Keep the trace map advisory. It should route attention, but a commit can still update a doc that
  was not the first trace-map hit when the model's evidence points elsewhere.
- Keep the tool token-bounded by default. Prefer commit metadata, path lists, stats, trace-map
  routing, and targeted doc section reads over full commit diffs or full design-doc reads.
- Keep all model inputs and outputs logged enough to audit decisions without storing secrets.
- Make the sweeper restartable. Cached classifier decisions and generated patch attempts should let
  a failed run resume without reprocessing every prior commit.
- Keep generated documentation factual and evidence-backed. When impact is uncertain, document the
  concrete behavior change and what should be watched in playtests rather than guessing strategy.
- Do not advance the reviewed checkpoint for an unmerged branch, a closed PR, a failed PR, or a
  branch that GitHub has accepted but whose head is not reachable from `origin/main`.
- Use a clean worktree and one `zvorygin/` branch for each implementation phase. When the sweeper
  itself opens sweep PRs, those branches must also use the `zvorygin/` prefix.
- During development, run the smallest focused checks for the changed surface. The full
  `./tests/run-all.sh` gate remains the PR merge authority.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- After each phase, the implementing agent must provide a handoff message with exact verification,
  behavior affected, remaining risks, next-phase guidance, and core manual test focus.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Suggested Execution

Run the doc audit and trace-map work first in a separate plan or task. After the trace map and
checkpoint exist, execute this plan one phase at a time:

```bash
scripts/phase-runner.sh --plan docdrift 1 --pr --wait
scripts/phase-runner.sh --plan docdrift 2 --pr --wait
scripts/phase-runner.sh --plan docdrift 3 --pr --wait
scripts/phase-runner.sh --plan docdrift 4 --pr --wait
```
