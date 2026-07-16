# Lab Scenario Selection and Authoring Plan

## Purpose

Let players and developers use labs as the normal workflow for selecting, creating, and exporting
reusable lab scenarios. The target product loop is: choose an existing scenario or blank lab, edit
authoritative game state with lab tools, fill in scenario metadata, validate it, and export JSON. This plan is about the
scenario selection and authoring process only; it does not add a public scenario database or broaden
labs into a visual asset iteration tool.

## Product Contract

- A user can open the lab route and choose from a visible catalog of bundled lab scenarios, or start
  from a blank map setup.
- A scenario in the catalog has stable id, title, description, tags, player count, map metadata, and
  the legacy setup JSON needed to restore it.
- The lab authoring panel can export the current authoritative lab setup, validate it as
  catalog-ready, and show clear blocking errors before export.
- Browser JSON import/export remains available for local iteration.

## Non-Goals

- Do not add durable scenario storage outside Git. No database-backed scenario library, moderation
  queue, sharing feed, or public browse/search service.
- Do not implement full authentication, user accounts, ownership, voting, comments, or publishing
  workflows.
- Do not migrate `/dev/scenario` or scripted dev-watch scenarios in this plan.
- Do not add server-side map authoring or persistence. Scenarios may reference only bundled maps whose
  metadata matches the then-current legacy setup validation path.
- Do not serialize exact runtime snapshots. Projectiles, command logs, transient events, fog
  projections, interpolation, and setup/teardown timers remain outside legacy setup JSON.
- Do not let the browser write server or repository paths.

## Phase Summaries

### [Phase 1 - Scenario Catalog and Selection](phase-1.md)

Replace the hardcoded one-scenario preset path with a small catalog layer for bundled lab scenarios.
The phase should keep `lategame` working, add a manifest or generated index with stable metadata,
and expose a bounded catalog API or start payload field that the browser can render before joining a
lab. The user-facing outcome is a lab entry flow that clearly offers existing scenarios and a blank
start without depending on hand-coded Rust enum entries for every new scenario.

### [Phase 2 - Authoring Metadata and Validation](phase-2.md)

Turn the existing import/export panel into an authoring surface for repo-ready scenarios. The phase
adds metadata fields, slug rules, validation feedback, and a dry-run path that proves the current
lab state can round-trip through legacy setup JSON and fit the catalog constraints. The user-facing
outcome is that authors can tell whether a scenario is catalog-ready before exporting it locally.

### [Phase 5 - Hardening, Docs, and Review Flow](phase-5.md)

Harden the end-to-end authoring path and document the supported workflow. This phase adds scenario
catalog guardrails, focused tests, design-doc updates, and a manual smoke covering
selecting an existing scenario, authoring a new one, exporting it, and reopening it locally.
The user-facing outcome is a documented scenario authoring
process with clear operational constraints.

## Overall Constraints

- Keep legacy setup JSON as the only reusable scenario setup format for this archived plan. If a
  phase needs a metadata wrapper, keep the scenario JSON itself valid under the existing
  import/restore path.
- Keep all privileged game-state changes through public `Game` lab APIs. Authoring export must not
  mutate sim internals or read a browser-supplied snapshot as authority.
- Reject duplicate slugs, oversized payloads, unknown maps, unsupported schema versions, and
  non-catalog fields during validation.
- Keep normal matches, replays, replay branches, dev scenarios, lobby behavior, and lab import/export
  stable unless a phase explicitly scopes a lab-only change.
- Keep protocol mirrors and docs together if a phase adds a new wire message, HTTP DTO, start
  capability, or lab result shape.
- Keep lab UI app-owned. `App` may own new catalog/authoring clients and pass small collaborators
  into `LabPanel`; `Match`, HUD, input, minimap, and renderer must not import authoring modules.
- Prefer deterministic JSON formatting and small manifests so exported files are readable.
- Use focused verification for each phase and rely on the PR `./tests/run-all.sh` gate for full
  coverage.
- A filtered test command only counts as verification when it actually runs matching tests.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed for the implementation PR, and be waited on until GitHub reports the phase merged
  and the head SHA is reachable from `origin/main`.
- After each phase, the implementing agent must provide a handoff message describing what changed,
  what the next agent should do, exact verification, and the core manual tests to run.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Implementation Process

Implement one phase at a time from fresh `origin/main`. Do not start the next phase from an assumed
merge; use `scripts/wait-pr.sh <pr>` after each implementation PR and verify the phase head is
reachable from `origin/main`.

For unattended executor passes after manual approval of this plan:

```bash
scripts/phase-runner.sh --plan labscenarios phase-1 phase-2 phase-5 --pr --wait
```
