# Phase 0: Freeze the Goal and Stop Digging

## Purpose

Stop trying to patch the current movement system directly. Treat it as a legacy system that may
contain useful parts, but do not assume its behavior or tests describe the game we want.

This phase is about creating permission to rebuild carefully.

## Decisions

- The current movement behavior is not the source of truth.
- The current movement tests are not automatically trustworthy.
- The new source of truth will be visible scenarios plus simple safety rules.
- The new system may live beside the old one until it is ready.
- Existing code can be reused only after it passes small, isolated checks.

## Keep as Possible Salvage

These areas may be valuable, but they still need to prove themselves:

- vehicle body shapes;
- oriented body collision checks;
- terrain and building legality checks;
- basic spatial lookup helpers;
- unit size and clearance constants.

## Treat as Suspicious

These behaviors may be deleted or replaced:

- waypoint skipping;
- hidden route lookahead;
- wall sliding;
- sidestep recovery;
- hardcoded unjam behavior;
- local steering repulsion;
- traffic turn bias;
- tests that lock in exact legacy timing or quirks.

## Done

- The team agrees the overhaul is a rebuild, not a cleanup pass.
- The old tests are kept temporarily but no longer treated as proof of good movement.
- The first scenario list is written down before implementation starts.
- No movement behavior has been rewritten yet.
