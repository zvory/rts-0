# Phase 2 - Cheap Commit Classifier

Status: draft.

## Goal

Add the token-cheap Codex pass that decides whether each considered commit should be ignored or
sent to the doc patch generator. The classifier should make a best-effort probabilistic decision,
not create a human triage queue.

## Scope

- Add configurable Codex CLI invocation for the cheap classifier.
- Forbid OpenAI Agents SDK, direct OpenAI API clients, API keys, and any API-billed fallback path.
- Feed the classifier only bounded inputs from Phase 1:
  - commit subject and body
  - changed paths
  - compact diff stats
  - docs touched
  - trace-map candidate docs
  - any prior cached decision for the same commit and prompt version
- Emit a structured decision record with:
  - commit SHA
  - decision: `move_on` or `update_docs`
  - likely authoritative docs to inspect or edit
  - short evidence note
  - prompt and Codex invocation metadata
- Cache classifier outputs so reruns do not spend tokens for unchanged inputs.
- Add a strict token and commit-count budget for one run, with clear failure output when the range
  is too large.
- Keep the default classifier path free of full commit diffs.

## Targeted Hunk Policy

Do not implement targeted hunk fetching by default. If Phase 2 evidence shows commit metadata is
too weak for useful decisions, add only a bounded opt-in mode where the same cheap classifier pass
may request small hunks for one commit under an explicit token cap. The stronger doc updater should
not independently browse broad diffs for every commit.

## Expected Touch Points

- `scripts/` for classifier orchestration.
- `tests/` for prompt input construction, decision parsing, cache-key stability, budget failures,
  and no-Codex fixture mode.
- Local ignored runtime output paths for classifier caches and reports.
- Documentation for required Codex CLI availability and safe dry-run usage.

## Out of Scope

- Do not edit docs in this phase.
- Do not open PRs.
- Do not advance the reviewed checkpoint.
- Do not add a manual-review decision state.
- Do not fail a sweep merely because a docs-impact decision is ambiguous; choose `move_on` or
  `update_docs`.

## Verification

Run focused tests with fixture Codex responses before any live Codex call:

```bash
node tests/docdrift_sweeper.mjs
node scripts/docdrift-sweep.mjs --classify --no-codex --fixture <fixture-name>
git diff --check
```

If a live Codex CLI smoke test is available on the operator machine, run it on a tiny bounded range
and record the range in the handoff. Do not use an API-key-backed smoke test.

## Manual Testing Focus

Inspect a classifier report across a recent small range that includes a gameplay change, a docs-only
change, and a mechanical refactor. Confirm that the output is easy to audit and that
`update_docs` decisions include enough evidence for Phase 3 without reading the whole diff.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must include the classifier command,
cache path, prompt/Codex invocation version, fixture coverage, any live Codex smoke result, and examples of
both `move_on` and `update_docs` records for the Phase 3 agent.
