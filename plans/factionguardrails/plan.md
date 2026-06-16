# Faction Guardrails Plan

## Purpose

Make faction drift prevention durable after the original faction plan moved to `plans/archive/`.
Restore the broken assumption checker, clarify active source-of-truth docs, and tighten parity so
catalog, protocol, config, client UI, replay, AI, and prediction surfaces cannot quietly disagree.

## Overall Constraints

- This is guardrail and refactor work, not new faction gameplay.
- Treat Rust rules/catalog data as authoritative and JS mirrors as checked projections.
- Preserve current live gameplay unless a phase explicitly identifies and fixes a current bug.
- Every protocol shape or compact-code change must update Rust protocol, server adapter if
  relevant, JS protocol mirror, design docs, and protocol parity tests in the same phase.
- Do not let archived plans become hidden active inputs unless a phase explicitly documents why.
- After each phase, provide a handoff naming verification results, remaining drift, and any manual
  faction/lobby behavior that should be tested.
- Implement, commit, merge to `main`, and push each phase before starting the next phase.

## Phase Summaries

### [Phase 1 - Checker Recovery And Source Of Truth](phase-1.md)

Fix `scripts/check-faction-assumptions.mjs` so it no longer fails on a moved `plans/faction`
directory. Decide which active docs define the current faction boundary and whether archived faction
files are historical only. This phase should force clarity before stronger ratchets land.

### [Phase 2 - Inventory Refresh And Boundary Ratchets](phase-2.md)

Refresh the faction architecture inventory so it matches the current repo rather than an older
single-faction snapshot. Separate playable, fixture-only, reserved/future, and historical faction
claims. Add checker anchors that fail on contradictory boundary language.

### [Phase 3 - Catalog Parity Hardening](phase-3.md)

Strengthen catalog parity so partial or accidental faction exposure fails loudly. Compare Rust
catalog dumps to client-exposed catalogs across ids, loadouts, buildables, trainables, research,
abilities, costs, compact codes, and command-card metadata where available. Fixture behavior should
remain explicit rather than inherited.

### [Phase 4 - Protocol And Config Drift Guardrails](phase-4.md)

Align protocol docs, Rust constants, JS constants, faction-bearing payload fields, compact codes,
and config mirror surfaces. Parity should catch `setFaction`, default faction ids, ability codes,
order-stage codes, kind codes, and compact-version drift. This phase should avoid gameplay changes
unless it exposes a real mismatch.

### [Phase 5 - Runtime Faction Surface Audit](phase-5.md)

Audit every runtime surface that accepts, defaults, exposes, or rejects faction ids. Lobby, AI
seats, quickstart, replay, replay branch, dev scenarios, self-play, prediction compatibility,
command cards, and hotkeys should follow one documented boundary contract. Prefer focused negative
tests over broad refactors.

### [Phase 6 - Gate Wiring And Selector Policy](phase-6.md)

Wire faction guardrails into the normal local gate and targeted suite selection. The assumption and
catalog parity checks should run where future agents will see them. Selector policy should choose
focused faction suites for faction-sensitive files without forcing live-server tests for docs-only
changes.

### [Phase 7 - Final Drift Review And Archive Policy](phase-7.md)

Perform a final audit for stale active references, contradictory boundary language, direct
special-case growth, and unguarded client/server faction surfaces. Establish clear archive policy
so scripts do not depend on moved plan files again. This phase should leave a concise guardrail map
for future faction work.

## Non-Goals

- Do not add a new playable faction or balance faction rosters.
- Do not rewrite archived historical plans except to remove active dependencies or document archive
  policy.
- Do not make client UI presentation fields Rust-authoritative unless they are real catalog facts.

## Handoff Rules

Each phase handoff must state the decided status of each catalog id touched, whether compact snapshot
version changed, where guard scripts run, and any remaining known drift.
