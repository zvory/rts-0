# Phase 6 - Regression Coverage, Docs, and Cleanup

Status: Pending.

## Goal

Harden the Tank Trap rollout with focused regression tests, documentation, and cleanup after the
feature is playable.

## Scope

- Add or consolidate focused server tests for:
  - infantry pass-through and same-tile standability
  - vehicle-body blocking for Tank, Scout Car, Command Car, Anti-Tank Gun, Mortar Team, and
    Artillery
  - two-tile gap behavior emerging from body/clearance rules
  - no vehicle path through diagonal-touching Tank Trap lines; if ordinary A* clearance still
    admits that path, vehicle pathing should use the Phase 2 0.5-tile Tank Trap blocker inflation
    fallback
  - under-construction vehicle blocking
  - ordinary buildings still blocking infantry and vehicles
  - elimination ignores Tank Traps
  - zero sight/fog reveal and remembered/scouted enemy display
- Add or consolidate focused client tests for:
  - worker build-card visibility after Training Centre
  - advisory placement policy
  - line preview and command dispatch
  - shallow and steep line drags that use diagonal-touching bridge sites instead of knight-move
    spacing
  - hotkey/profile behavior if the new build slot affects hotkey tests
  - renderer stability for live and remembered Tank Traps
- Add local dev scenario launches for constructible Tank Trap blocker layouts:
  - horizontal, vertical, and diagonal Tank Trap lines built through normal production/build
    commands after the required Training Centre setup
  - each layout should include at least one vehicle-body unit and one infantry unit attempting to
    cross the trap line
  - each scenario should make it easy to verify that vehicles cannot pass through the closed Tank
    Trap line while infantry can path through and stand on trap tiles
- Update design docs and capsules as needed:
  - `docs/design/balance.md`
  - `docs/design/protocol.md`
  - `docs/design/server-sim.md` for movement-class static blockers
  - `docs/design/client-ui.md` if line placement adds a new input collaborator
  - relevant `docs/context/*.md` section lists if structure shifts
- Search for obsolete TODOs, comments, or old assumptions that say every building blocks every unit
  or every build placement is a single-site interaction.
- Collect factual patch-note bullets.

## Expected Deliverables

- The feature's highest-risk behavior is covered by targeted tests.
- Dev scenarios cover producible horizontal, vertical, and diagonal Tank Trap configurations where
  vehicles are blocked and infantry can pass.
- Docs describe Tank Trap stats, protocol kind, vehicle-only blockers, line placement, and
  elimination exclusion.
- Any rough art/sound/AI/repair/cancel follow-ups are explicitly recorded rather than hidden.
- The phase document is marked done in the implementation commit.

## Out of Scope

- Balance retuning beyond the specified 15 steel, 200 HP, 10-second build time.
- AI strategic building or counterplay.
- Final art or sound pass.
- Implementing repair, cancel, or salvage mechanics.

## Verification

- Run targeted Rust and Node suites selected by the files changed in this phase.
- Run protocol parity checks if kind-code or protocol docs changed in previous phases and were not
  already verified.
- Run client architecture checks if a client collaborator was added in Phase 5.
- Run broader local gates only if the implementation phase scope or commit hook requires them.

## Manual Testing Focus

Run one local match flow: build Training Centre, construct single and line Tank Traps, watch
engineers distribute work, verify infantry can cross, verify vehicles need a wide enough gap, verify
shallow or steep dragged lines do not leave knight-move diagonal gaps, destroy some traps, and
confirm a player with only Tank Traps is eliminated. Also open the horizontal, vertical, and
diagonal constructible Tank Trap dev scenarios and confirm every vehicle test unit is blocked while
the infantry test unit crosses the same layout.

## Handoff Expectations

The final handoff must list tests run, manual testing performed or skipped, patch-note bullets,
remaining rough edges, and whether the worktree branch was merged to `main` and pushed.
