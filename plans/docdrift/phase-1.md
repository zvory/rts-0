# Phase 1 - Sweep Contract and Dry Run

Status: draft.

## Goal

Create the deterministic non-model foundation for the documentation drift sweeper. The result
should let an operator run a dry sweep from the reviewed checkpoint to `origin/main`, inspect the
commit set, and see trace-map routing without spending model tokens or editing files.

## Scope

- Add a local sweep command or script under the repo's existing script conventions.
- Load the reviewed checkpoint from a single ref, tag, or small state file chosen during
  implementation.
- Load the preexisting trace map produced by the separate doc audit.
- Fetch the list of non-merge commits between the checkpoint and `origin/main`.
- For each considered commit, collect:
  - commit SHA
  - subject and body
  - author date
  - changed path list
  - compact diff stats
  - whether `docs/design/*` or `docs/context/*` changed
  - trace-map candidate docs
- Produce a stable dry-run report in a human-readable format and a structured machine-readable
  format.
- Exit cleanly when there are no commits to sweep.

## Expected Touch Points

- `scripts/` for the operator command.
- `tests/` for focused unit or fixture tests around checkpoint ranges, merge filtering, and report
  shape.
- The committed trace-map path from the doc audit.
- `docs/context/testing.md` or an adjacent docs capsule only if a new reusable test command needs
  to be discoverable.

## Out of Scope

- Do not call any model in this phase.
- Do not edit authoritative design docs.
- Do not create sweep PRs.
- Do not advance the reviewed checkpoint.
- Do not invent the trace map if the separate doc audit has not provided it.

## Verification

Run focused checks for the new script and fixtures. Likely commands:

```bash
node tests/docdrift_sweeper.mjs
node scripts/docdrift-sweep.mjs --dry-run --base <test-sha-or-ref> --head origin/main
git diff --check
```

Use the actual filenames chosen during implementation. The dry-run command should be tested against
a bounded commit range so the result is stable enough to review.

## Manual Testing Focus

Run the dry sweep from a recent checkpoint to `origin/main` and inspect the report. Confirm that the
commit list, merge filtering, docs-touched detection, diff stats, and trace-map candidate docs are
understandable without opening every commit.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must name the checkpoint storage
mechanism, the trace-map file path, the dry-run command, the structured report format, exact
verification results, and any assumptions Phase 2 must preserve when adding model classification.
