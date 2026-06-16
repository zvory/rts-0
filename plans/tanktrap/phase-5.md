# Phase 5 - Line Placement UX and Build Exposure

Status: Pending.

## Goal

Expose Tank Trap construction to players through the worker build card and implement the line-drag
workflow.

## Scope

- Add Tank Trap to the default worker build-card sequence in the next open slot.
- Add a focused client line-placement helper/collaborator rather than embedding all line math in
  the main input class.
- Implement Bresenham-style tile line generation:
  - start tile included
  - every other tile used for trap sites
  - end tile included only when it lands on the cadence
  - invalid positions skipped while later positions remain eligible
- Add line preview rendering that shows valid and invalid trap positions while dragging.
- Preserve normal single-click behavior as a one-site line whose start tile is included.
- Implement command dispatch using existing `build` commands unless Phase 0 explicitly required a
  protocol change:
  - without Shift, send at most one immediate build command per selected worker, assigning the first
    valid sites from drag start toward drag end
  - use one selected worker id per immediate command so 12 workers and 20 valid sites send exactly
    the first 12 sites
  - with Shift, send additional queued build commands for remaining valid sites using the existing
    queued worker-distribution semantics
  - keep current affordability-on-arrival semantics; do not reserve steel for the whole line
- Ensure right-click/Escape/blur/Shift-release interactions cleanly cancel or preserve placement in
  the same style as current placement targeting.
- Add focused client tests for:
  - line tile generation and every-other-tile cadence
  - invalid-site skipping
  - non-Shift command count and worker/site assignment
  - Shift extra-site queued command behavior
  - build-card descriptor slot, hotkey, affordability, and requirement disabled states

## Expected Deliverables

- Engineers can select Tank Trap from the worker build card after Training Centre.
- Left-click builds one Tank Trap; left-drag builds an every-other-tile line.
- Non-Shift drag sends no more sites than selected workers.
- Shift drag queues enough additional standard build commands to finish the valid line under
  existing server queue semantics.
- The client does not introduce a new authoritative placement rule that can disagree permanently
  with the server.

## Out of Scope

- New wire command shape unless Phase 0 revised this plan.
- AI usage.
- Sound and final art polish.
- New cancel, repair, or salvage UI.

## Verification

- Run focused Node tests for input placement, command-card descriptors, and client contracts touched
  by this phase.
- Run `node scripts/check-client-architecture.mjs` if the line-placement helper changes module
  structure.
- If protocol shape changes, run protocol parity checks and update Phase 6 docs accordingly.

## Manual Testing Focus

In a local match, select multiple engineers, build a Tank Trap line without Shift, and confirm only
the first selected-worker-count valid sites receive commands. Repeat with Shift and confirm later
sites are queued/distributed, invalid terrain is skipped, and vehicles cannot drive through the
completed line unless a wide enough gap exists.

## Handoff Expectations

The handoff must describe the line helper API, command dispatch rules, build-card slot/hotkey,
tests run, and any manual interaction issues Phase 6 should cover in final smoke testing.
