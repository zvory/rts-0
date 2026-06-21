# One-Time Documentation Drift Reset

## Purpose

Run a slow, serial, evidence-bound cleanup of stale gameplay documentation after the current
commit-range documentation sweeper has finished. This plan is not the recurring drift sweeper; it
is a one-time reset pass over current game sectors so obvious stale facts can be removed or fixed
before future sweeps operate from a cleaner baseline.

## Operating Model

- Execute with `scripts/phase-runner.sh --pr --wait` so each sector lands through an owned PR,
  merges to `main`, and becomes the base for the next sector.
- Start only after the overnight wrapper sleeps and refreshes from `origin/main`; another docs
  drift sweep may have already fixed some stale documentation by then.
- Keep every phase evidence-bound. Compare active docs and wiki/generated stats against current
  source-of-truth code, then patch only confirmed stale facts.
- Prefer minimal factual edits over prose rewrites. If a claim is ambiguous, report it in the
  handoff instead of inventing replacement behavior.
- If an audit finds no stale content, mark only that phase document done and commit the no-op audit
  result with a handoff explaining what was checked.

## Current High-Risk Drift

- Player-facing references to "Steelworks" are stale unless the text is deliberately naming the
  internal protocol/stable id `steelworks`. The current player-facing building name is "Gun Works";
  protocol docs may still mention `steelworks` as a stable wire identifier when that distinction is
  explicit.
- `/wiki/stats` is generated from Rust rules definitions and faction catalogs, so generated labels
  and extracted stats need to be checked alongside Markdown docs.
- Archived plans, raw replay JSON, and incident evidence are historical records. Do not edit them
  merely because they contain old names or old protocol ids.

## Phase Summaries

### [Phase 1 - Obsolete Names and Generated Stats](phase-1.md)

Sweep active docs and `/wiki/stats` generation for retired player-facing names, especially
Steelworks versus Gun Works. Fix high-confidence stale labels and prerequisites in generated stats,
wiki output, and active docs while preserving internal stable ids where they are intentionally part
of protocol or compatibility documentation. Leave historical artifacts alone.

### [Phase 2 - Roster, Stats, and Balance Tables](phase-2.md)

Compare authoritative unit, building, upgrade, ability, cost, supply, sight, movement, and combat
numbers against active balance docs and generated wiki stats. Patch stale visible stats and
requirements that are confirmed by current Rust rules, faction catalogs, and client config mirrors.
Do not tune gameplay values in this phase.

### [Phase 3 - Economy, Production, and Tech Tree](phase-3.md)

Audit economy flow, resource handling, construction, build prerequisites, training prerequisites,
research locations, command-card unlock text, and production chains. Fix docs that describe old tech
paths, old prerequisite buildings, or old resource/supply behavior. Keep code changes limited to
generated reference labels or deterministic checks needed to keep the docs accurate.

### [Phase 4 - Combat, Abilities, Fog, and Orders](phase-4.md)

Audit combat behavior, targeting, ranges, cooldowns, projectiles, special abilities, order queues,
fog visibility, and privacy-sensitive event documentation. Fix active docs that conflict with the
current sim, rules, protocol, or hardening invariants. Do not change combat behavior.

### [Phase 5 - Client UI, Prediction, Renderer, and Lab](phase-5.md)

Audit user-facing client docs for HUD, command cards, input, hotkeys, prediction, renderer feedback,
minimap, settings, audio, unit lab, and debug/lab tooling. Fix stale UI behavior claims and stale
entry points that would mislead a future agent or player. Keep visual and UI implementation changes
out of scope unless they are necessary for generated wiki/stat correctness.

### [Phase 6 - Lobby, Protocol, Replays, and Match History](phase-6.md)

Audit lobby flow, WebSocket protocol docs, compact protocol references, room lifecycle, replay and
branching behavior, match-history persistence, and deployment-facing API claims. Fix active docs
that describe old message shapes, old lobby capabilities, old replay fields, or old match-history
ownership. Preserve wire compatibility identifiers unless code actually removed them.

### [Phase 7 - AI, Testing, Dev Scenarios, and Tooling](phase-7.md)

Audit AI profiles, self-play expectations, dev scenarios, testing docs, and script/tooling docs.
Fix stale profile names, scenario behavior, test commands, and coverage claims that conflict with
current code. Do not retune AI or rewrite test architecture.

### [Phase 8 - Final Consistency Sweep](phase-8.md)

Run a final active-doc consistency pass after the sector fixes have merged. Search for remaining
retired terms and contradiction clusters, run focused docs/wiki checks, update capsules only when
their pointers are stale, and leave a compact handoff describing any unresolved uncertainty.

## Overall Constraints

- Active source-of-truth docs live in `docs/design/*.md`; `docs/context/*.md` are routing capsules.
  Update capsules only when the entry points, section lists, or routing text are stale.
- Do not edit `plans/archive/**`, raw replay JSON, incident logs, or other historical evidence
  unless the phase explicitly identifies active guidance embedded there.
- Do not rename protocol ids, Rust enum variants, compact kind codes, database fields, or replay
  fields just because their player-facing label changed.
- For player-facing building and unit names, prefer the current client/rules labels and current
  wiki output. Internal ids should be wrapped in backticks and described as internal/protocol ids.
- Keep docs factual. If strategic impact is uncertain, say what changed and what should be watched
  in playtests.
- Use `rg` and `fd` for searches.
- Run the smallest focused verification for each phase. Common checks are
  `node scripts/check-docs-health.mjs`, `node scripts/check-wiki.mjs`,
  `node scripts/check-faction-catalog-parity.mjs`, and `git diff --check`.
- Each phase must mark its own phase file done only after its changes are committed.
- Each phase must provide a handoff with files changed, verification, gameplay impact, next
  executor notes, and manual test notes.

## Suggested Execution

The wrapper sleeps first, then creates or refreshes a clean temporary clone, pulls `origin/main`,
and runs each phase serially with PR waiting:

```bash
scripts/run-docs-reset-overnight.sh
```

Useful overrides:

```bash
DOCS_RESET_START_DELAY_SECONDS=3600 scripts/run-docs-reset-overnight.sh
DOCS_RESET_PHASES="3 4 5" scripts/run-docs-reset-overnight.sh
DOCS_RESET_MODEL=gpt-5.5 scripts/run-docs-reset-overnight.sh
```
