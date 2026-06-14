# Phase 5 - Broader Agent Legibility Design Gate

Status: Design blocked.

## Objective

Do not implement broader agent-legibility tooling in this phase. Phase 1.5 already covers the
approved static, non-editable map editor atlas tab. The goal here is to explicitly pause before
building any additional SVG, PNG, Markdown, JSON-report, export, route-debug, or agent-facing map
inspection output because the right format and workflow for those tools are not yet known.

## Scope

- Start with a user design discussion before writing implementation code beyond the Phase 1.5
  editor tab.
- Clarify what problem additional agent-legibility tooling should solve after the atlas-backed AI
  behavior exists:
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

- Do not generate SVG overlays or exports.
- Do not generate PNG overlays or exports.
- Do not add a Markdown map report.
- Do not add another browser map-inspection UI beyond the Phase 1.5 static atlas editor tab.
- Do not add agent-facing artifact generation to tests or handoffs.
- Do not make any code changes for additional agent-legibility tooling without a follow-up
  user-approved plan.

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

The handoff must say whether the user approved any additional agent-legibility tooling direction.
If no direction was approved, the handoff must explicitly state that tooling beyond the Phase 1.5
static atlas editor tab remains blocked and must not be implemented by executor automation.

## Player-Facing Outcome

No player-facing change. This phase prevents premature extra tooling work from distracting from
the atlas-backed routing foundation.
