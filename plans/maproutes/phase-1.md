# Phase 1: Static analysis scaffold

Status: Pending

## Goal

Create the first AI-owned static map-analysis scaffold with deterministic data and tests, without
protocol, client, or gameplay behavior changes.

## Scope

- Add a map-analysis module under `server/crates/ai/src/ai_core/` or another locally consistent
  AI-owned path.
- Build data types for passability, clearance, components, base/start mapping, resource cluster
  mapping, and a compact debug snapshot.
- Cache analysis on `AiController` or an equivalent AI-owned boundary keyed by static map/start
  identity so it is not recomputed every decision tick.
- Keep `AiObservation` behavior-compatible unless a small additive field is clearly needed for
  tests.
- Add focused unit tests for Default, Low Econ, and No Terrain map inputs where practical.

## Non-goals

- Do not add spectator/client overlay rendering.
- Do not change command emission, AI decisions, pathing, combat, economy, or balance.
- Do not change wire protocol unless unavoidable; this phase should normally avoid it.

## Expected touch points

- `server/crates/ai/src/ai_core/`
- `server/crates/ai/src/live.rs`
- `server/crates/ai/src/selfplay/player_view.rs`
- `server/assets/maps/*.json` only as read-only fixtures
- `docs/design/ai.md` if a new AI-owned static-analysis contract is documented

## Verification

- Run focused Rust tests for the `rts-ai` crate or the specific map-analysis module.
- Include deterministic assertions for map size, passable component counts, base-to-component
  mapping, and clearance sanity.
- Confirm existing AI self-play tests still compile if touched surfaces are public to self-play.

## Manual testing focus

No manual gameplay test is required beyond confirming there is no behavior change. If a debug dump
or test-only print helper is added, inspect Default once to ensure the scaffold data is readable.

## Handoff

The handoff must summarize the data model, cache key, and fixture evidence, and call out any
analysis limitations intentionally left for Phase 3.
