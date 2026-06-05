# Game-Backed Dev Scenario Viewer

This plan is split into phases so the first useful pathfinding debugger can ship without building
the full scenario platform up front.

## Goal

Add a dev-only scenario viewer that lets a human inspect authored simulation situations in the
normal game renderer. The first target is the ignored scout-car snaking corridor timing scenario.

This is not a plan to fix `scout_car_snaking_corridor_clear_times`. The goal is to make the
behavior visible and reproducible so fixes can be investigated against actual server behavior.

## Why Game-Backed

Scenarios should use `Game` wherever possible:

- the renderer receives the same `start` and `snapshot` messages it already understands;
- combat events, death events, target tracers, resource deltas, sounds, fog-off spectator views, and
  interpolation keep working through the existing client path;
- scenario behavior stays aligned with the public simulation seam instead of growing a parallel
  visualization simulator;
- recorded command logs and snapshot artifacts can be added later without changing the viewer model.

Direct service-level runners should exist only as an escape hatch for tests that intentionally
exercise internals below the `Game` seam and cannot be honestly represented by a custom `Game`.

## Phase Index

1. [Phase 1: Minimal Pathfinding Viewer](game-backed-dev-scenarios/phase-1-minimal-pathfinding-viewer.md) - done
2. [Phase 2: Scenario Setup API](game-backed-dev-scenarios/phase-2-scenario-setup-api.md)
3. [Phase 3: Scenario Framework](game-backed-dev-scenarios/phase-3-scenario-framework.md)
4. [Phase 4: Debug Overlays and Artifacts](game-backed-dev-scenarios/phase-4-debug-overlays-and-artifacts.md)

## Cut Line

Phase 1 is the minimum viable system: one hardcoded game-backed scout-car corridor scenario, viewed
through the real client. Do not build generic scenario traits, artifact recording, combat examples,
or pathfinding overlays until Phase 1 proves useful.

## Non-Goals

- Do not fix scout-car movement as part of the viewer.
- Do not expose arbitrary map/unit spawning to clients.
- Do not create a second renderer or a standalone canvas UI for the main path.
- Do not change normal match protocol semantics for non-dev rooms.
- Do not make Bazel edits; if generated build metadata is ever needed, use the repo's Gazelle flow.

## Verification

For this documentation split, no runtime tests are required.

For eventual implementation phases, use each phase file's verification section.
