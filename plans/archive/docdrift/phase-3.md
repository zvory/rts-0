# Phase 3 - Doc Patch Generator

Status: done.

## Goal

Add the stronger Codex pass that turns `update_docs` decisions into minimal authoritative design-doc
patches. The output should be one clean docs branch for the sweep, with edits that are factual,
scoped, and useful to future agents.

## Scope

- Read the Phase 2 decision records marked `update_docs`.
- For each decision, select relevant authoritative design docs using the trace map and classifier
  targets.
- Load only targeted doc sections when possible, not entire large docs by default.
- Provide the stronger Codex invocation with:
  - selected decision record
  - compact commit metadata and evidence note
  - relevant doc sections
  - repo documentation rules
  - instruction to make factual minimal updates
- Forbid OpenAI Agents SDK, direct OpenAI API clients, API keys, and any API-billed fallback path.
- Apply Codex-generated patches to `docs/design/*.md` first.
- Update `docs/context/*.md` only when the authoritative doc's section structure or entry points
  change.
- Accumulate all doc edits into one sweep branch.
- Make the operation idempotent enough that rerunning after a partial failure does not duplicate
  bullets or repeatedly rewrite the same paragraph.

## Commit Shape

The implementation may either create one aggregate docs commit or one commit per source commit.
Both are acceptable if the sweep branch remains a single owned PR and the final diff is readable.
Commit messages should identify that the changes are doc-drift cleanup and should include the
source commit SHAs or report path when practical.

## Expected Touch Points

- `scripts/` for patch generation and branch preparation.
- `tests/` for patch parsing, idempotency fixtures, doc-section selection, and no-Codex fixture
  mode.
- `docs/design/*` only in generated sweep runs, not as part of this phase implementation unless
  documenting the new sweeper behavior itself requires it.
- `docs/context/testing.md` or a small operator doc if new commands need durable documentation.

## Out of Scope

- Do not rewrite design docs broadly.
- Do not update balance mirrors, protocol mirrors, or gameplay code.
- Do not mark generated impact as certain when the evidence only supports a concrete behavior
  description.
- Do not advance the reviewed checkpoint.
- Do not make generated docs bypass normal review, PR checks, or branch protection.

## Verification

Run fixture-backed patch generation first:

```bash
node tests/docdrift_sweeper.mjs
node scripts/docdrift-sweep.mjs --generate-docs --no-codex --fixture <fixture-name>
node scripts/check-wiki.mjs
git diff --check
```

If Codex CLI is available on the operator machine, run a live generation smoke test on a tiny
throwaway range and inspect the resulting docs diff before committing. Do not use an API-key-backed
smoke test.

## Manual Testing Focus

Review the generated docs diff as a future implementation agent. Confirm that the changed design
doc would have helped someone understand the current behavior without opening the source commit,
and that the sweeper did not add speculative strategy claims or broad prose churn.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must include the generation command,
where generated reports are stored, how to inspect source commit evidence for a docs edit, exact
verification results, and any known weak spots in section targeting or idempotency.
