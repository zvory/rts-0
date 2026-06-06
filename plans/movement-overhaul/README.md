# Movement Overhaul Plan

This plan rebuilds vehicle movement from first principles while treating the current movement
behavior as legacy code. The goal is not to understand and refactor every existing heuristic. The
goal is to create a smaller, visible, explainable movement system next to the old one, prove it in
core scenarios, and only keep old pieces that pass isolated checks.

The movement system is a core design feature of the game. It should feel physical, readable, and
reliable:

- vehicles should leave sensible clearance around corners;
- tanks, scout cars, and AT guns should share the same clearance rules;
- scout cars should still feel like cars, not tanks;
- units should handle chokes and traffic without turning into walls or each other;
- every strange movement choice should be explainable at a specific tick.

## Guiding Rules

- Do not start by deleting everything.
- Do not start by trusting the current tests.
- Do not try to understand the whole legacy movement system up front.
- Rebuild the model in a small scenario lab.
- Salvage old code only after it proves itself in isolation.
- Prefer visual, repeatable scenarios over abstract confidence.
- Keep early behavior simple even if it looks less clever.
- Add cleverness only after the basics are explainable.

## Phases

- [Phase 0 - Freeze the Goal and Stop Digging](phase-0-freeze-the-goal.md)
- [Phase 1 - Build the Movement Lab](phase-1-movement-lab.md)
- [Phase 2 - Prove Static Bodies and Clearance](phase-2-bodies-and-clearance.md)
- [Phase 3 - Create the New Movement System](phase-3-new-movement-system.md)
- [Phase 4 - Single-Vehicle Movement](phase-4-single-vehicle-movement.md)
- [Phase 5 - Traffic and Chokes](phase-5-traffic-and-chokes.md)
- [Phase 6 - Replace, Delete, and Lock In](phase-6-replace-delete-lock-in.md)

## What Success Looks Like

A developer can open a movement scenario, scrub to an exact tick, select a unit, and understand why
it moved, waited, turned, reversed, or failed. The same scenarios run as tests with simple pass/fail
metrics. Once the new system handles the core scenarios, old movement heuristics can be deleted with
confidence instead of fear.
