# Phase 3 - Retired Pending Specification

## Phase Status

- [x] Done. Retired without implementation.

## Why It Was Retired

The former specification combined fog revision caching with minimap blips, health/selection
geometry, and occupied-trench overlays. It was written before function-level client CPU evidence
could distinguish a dominant hotspot from adjacent speculative cache work.

Fresh profiling made fog an evidence-backed investigation target but did not validate the old
bundle or its order relative to rigs, diagnostics, copying, and Pixi presentation. Its implementation
scope, targets, and ordered work were removed rather than carried into a data-guided plan.

## Replacement Gate

Do not execute this phase with `phase-runner`. Capture the canonical snapshot-stream profile and an
active-player profile when relevant, inspect their ranked functions and source, then create only the
phases justified by the current measurements.
