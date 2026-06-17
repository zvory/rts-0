# Phase 5 - Line Placement UX and Build Exposure

Status: Done.

## Goal

Expose Tank Trap construction to players through the worker build card and implement the line-drag
workflow.

## Scope

- Add Tank Trap to the default worker build-card sequence in the next open slot.
- Add a focused client line-placement helper/collaborator rather than embedding all line math in
  the main input class.
- Keep placement drag distinct from box selection and camera drag paths. Starting a Tank Trap
  placement drag must not accidentally trigger selection drag, and cancel/blur paths should cleanly
  return ownership to the existing input modes.
- Implement Bresenham-style tile line generation with no diagonal vehicle gaps:
  - start tile included
  - same-row or same-column trap sites may have one empty tile between them
  - diagonal trap sites should touch corner-to-corner
  - consecutive emitted trap sites must not be a knight's move apart (`abs(dx), abs(dy)` of `2,1`
    or `1,2`)
  - when a shallow or steep Bresenham line would otherwise emit a knight-move pair, insert or choose
    the diagonal-touching bridge site that keeps the line closed to vehicles
  - end tile included only when it lands on the no-gap cadence
  - invalid positions skipped while later positions remain eligible
- Add line preview rendering that shows valid and invalid trap positions while dragging.
- Preserve normal single-click behavior as a one-site line whose start tile is included.
- Implement command dispatch using existing `build` commands unless Phase 0 explicitly required a
  protocol change:
  - send at most one immediate build command per selected worker, assigning the first valid sites
    from drag start toward drag end
  - use one selected worker id per immediate command so 12 workers and 20 valid sites send exactly
    12 immediate commands for the first 12 sites
  - when valid sites remain after the immediate assignments, send additional queued build commands
    for sites `n+1..m` against the selected worker set using the existing queued
    worker-distribution semantics, even when Shift is not held
  - Shift may still preserve placement mode according to existing build-placement behavior, but it
    is not required for overflow sites in a drawn Tank Trap line
  - keep current affordability-on-arrival semantics; do not reserve steel for the whole line
  - accept that later queued sites may fail independently for resources, blockers, queue limits, or
    arrival-time validation; surface feedback through existing command notices
- Ensure right-click/Escape/blur/Shift-release interactions cleanly cancel or preserve placement in
  the same style as current placement targeting.
- Add focused client tests for:
  - line tile generation for orthogonal one-tile gaps, diagonal touching, and shallow/steep lines
    that must avoid knight-move spacing
  - invalid-site skipping
  - immediate command count and worker/site assignment
  - immediate dispatch emits one single-worker `build` command per selected worker/site rather than
    one multi-worker command that the ordinary planner could collapse to one closest builder
  - non-Shift overflow-site queued command behavior for a line with more valid sites than selected
    workers
  - Shift placement preservation without changing overflow-site queueing
  - build-card descriptor slot, hotkey, affordability, and requirement disabled states

## Expected Deliverables

- Engineers can select Tank Trap from the worker build card after Training Centre.
- Left-click builds one Tank Trap; left-drag builds a no-diagonal-gap line.
- A drag with `n` selected workers and `m` valid sites sends up to `n` immediate builds and
  automatically queues sites `n+1..m` under existing server queue semantics, even without Shift.
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

In a local match, select multiple engineers, build a Tank Trap line longer than the selected worker
count without Shift, and confirm the first selected-worker-count valid sites receive immediate
commands while later valid sites are queued/distributed across those workers. Repeat with Shift and
confirm Shift only preserves placement behavior; overflow site queueing remains the same, invalid
terrain is skipped, and vehicles cannot drive through the completed line unless a wide enough gap
exists. Include a shallow or steep diagonal drag and confirm the preview never places consecutive
Tank Traps a knight's move apart.

## Handoff Expectations

The handoff must describe the line helper API, command dispatch rules, build-card slot/hotkey,
tests run, and any manual interaction issues Phase 6 should cover in final smoke testing.
