# Phase 5 - Agent Legibility Design Gate

Status: Design blocked.

## Objective

Do not implement agent-legibility tooling in this phase. The goal is to explicitly pause before
building SVG, PNG, Markdown, JSON-report, or other agent-facing map inspection output because the
right format and workflow are not yet known.

## Scope

- Start with a user design discussion before writing implementation code.
- Clarify what problem agent-legibility tooling should solve after the atlas-backed AI behavior
  exists:
  - helping agents avoid false map claims during planning
  - helping humans review map topology changes
  - helping AI developers debug route selection
  - producing artifacts for tests or handoffs
- Decide whether the output should be visual, textual, structured, interactive, or some combination
  of those formats.
- Decide who the primary consumer is: user, implementation agents, review agents, future live AI
  diagnostics, or map authors.
- Decide whether the tooling belongs in Rust, Node, browser UI, dev-only server routes, or an
  external script.
- Decide what generated artifacts, if any, may be committed. Avoid committing large generated
  images or brittle snapshots without explicit approval.
- After the design discussion, write a new implementation plan if the user approves tooling work.

## Explicit Non-Goals

- Do not generate SVG overlays.
- Do not generate PNG overlays.
- Do not add a Markdown map report.
- Do not add a browser map-inspection UI.
- Do not add agent-facing artifact generation to tests or handoffs.
- Do not make any code changes for agent-legibility tooling without a follow-up user-approved plan.

## Expected Touch Points

None for implementation. A design-only follow-up may update:

- `plans/airouting/` or a new one-word plan directory
- `docs/design/ai.md`
- `docs/design/server-sim.md`
- map authoring documentation, if such documentation exists by then

## Verification

No automated verification is expected because this is a design gate. If a follow-up design brief is
created, verify only that it captures the user's decisions and does not sneak in implementation work.

## Manual Testing Focus

None. This phase is intentionally blocked until the user leads a design discussion.

## Handoff Expectations

The handoff must say whether the user approved any agent-legibility tooling direction. If no
direction was approved, the handoff must explicitly state that tooling remains blocked and must not
be implemented by executor automation.

## Player-Facing Outcome

No player-facing change. This phase prevents premature tooling work from distracting from the
atlas-backed routing foundation.
