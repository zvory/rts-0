# Phase 3 - Extraction Candidate Ranking

Status: done.

## Goal

Convert hotspot metrics and responsibility maps into a ranked backlog of cleanup candidates. The
output should make the next cleanup decision obvious: which extractions are safe mechanical
decomposition, which require design work, and which should be deferred.

## Scope

- Read `plans/hotspots/baseline.md` and `plans/hotspots/responsibility-map.md`.
- Create `plans/hotspots/extraction-candidates.md`.
- Rank candidates by:
  - expected cognitive-load reduction;
  - current churn and recency;
  - current file size;
  - contract centrality and coupling;
  - risk of behavior change;
  - risk of breaking history-aware hotspot tracking after file splits;
  - ownership conflicts with active plans;
  - verification confidence.
- For each candidate, document:
  - proposed extraction or cleanup target;
  - exact source files likely touched by a later cleanup plan;
  - what should remain stable;
  - what tests or checks would verify behavior;
  - manual testing focus;
  - expected player-facing impact, if any;
  - why the candidate is ranked where it is.
- Identify no-go candidates where splitting the file would make the codebase look cleaner without
  reducing real coupling.
- Identify candidates that should become their own follow-up `plans/<name>/` phased plans.

## Expected Touch Points

- `plans/hotspots/extraction-candidates.md`
- `plans/hotspots/phase-3.md`

Do not modify runtime source, tests, protocol files, client modules, CSS, or design docs in this
phase.

## Candidate Types To Consider

- Splitting `tests/client_contracts.mjs` by contract area.
- Extracting command-service submodules from
  `server/crates/sim/src/game/services/commands.rs`.
- Moving room-task responsibilities only after active room cleanup lands and only if the map shows
  stable seams.
- Splitting client match shell responsibilities out of `client/src/match.js` only where dependency
  injection and teardown boundaries stay clear.
- Grouping protocol and balance mirror cleanup at the contract level instead of chasing single-file
  line count.
- Splitting large sim or AI test files by behavior family if test setup can stay shared and
  readable.
- Deferring CSS modularization unless the responsibility map shows specific repeated UI domains
  that are safer to extract than to leave in `client/styles.css`.

## Analysis Questions

- What is the first cleanup plan that gives the best return with the least risk?
- Which candidates are pure file organization, and which would require behavior or API changes?
- Which candidates would improve future LLM context loading the most?
- Which candidates would hurt Git history unless the follow-up plan uses careful move/copy-aware
  review?
- What should be measured again after each cleanup lands?

## Verification

- `git status --short`
- `git diff --check`
- Cross-check every candidate against the Phase 1 evidence and Phase 2 responsibility map.
- Confirm that no candidate requires editing files owned by an active plan unless it is explicitly
  marked deferred.

## Manual Review Focus

Review whether the ranking is useful enough to choose the next implementation plan. It should not
read like a generic refactor wish list; every candidate should have a reason tied to current churn,
size, coupling, or responsibility-map evidence.

## Handoff

After implementation, mark this phase done and summarize the top ranked cleanup candidates, the
recommended first follow-up plan, and the candidates that should be avoided or deferred. Tell the
next phase which metrics and group mappings need to be repeatable for future comparisons.
