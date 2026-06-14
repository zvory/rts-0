# Phase 5 - Diagnostics and Validation

Status: Planned.

## Objective

Harden the new terrain routing system with diagnostics, self-play validation, and documentation so it
can support future AI movement work. This phase should make route decisions observable and
regression-resistant without expanding into a full tactical planner.

## Scope

- Add compact route diagnostics to AI manager traces or self-play scorecards: selected corridor id,
  route score, main blockers, hot-route status, and first Scout Car harassment route metadata.
- Add focused self-play or scenario validation around the known Default map right-side spawn case.
  Prefer a short deterministic harness that checks route commands and progress over a long
  20,000-tick balance run.
- Add a route snapshot or unit test fixture for the Default map candidate scores so map edits or
  scoring changes make route behavior changes visible in review.
- Verify that route traces stay diagnostic-only and do not affect deterministic replay command
  logs.
- Update `docs/design/ai.md` with the final terrain routing architecture, ownership boundaries,
  limitations, and future extension points for army movement, scouting, or expansion path safety.
- Refresh `docs/context/server-sim.md` only if routing files or design-section pointers changed
  enough that the capsule would send future agents to stale locations.
- Collect factual gameplay patch-note bullets for the final implementation summary.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/trace.rs`
- `server/crates/ai/src/selfplay/replay.rs`
- `server/crates/ai/src/selfplay/tests.rs` or a focused self-play scenario module
- Route evaluator and harassment tests from earlier phases
- `docs/design/ai.md`
- `docs/context/server-sim.md` only if section pointers need refresh

## Verification

Run the focused checks added by this phase plus the route and harassment test filters:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai routing
cargo test --manifest-path server/Cargo.toml -p rts-ai harassment
cargo test --manifest-path server/Cargo.toml -p rts-ai selfplay
```

If a live Node or browser check is added for the manual route case, keep it narrow and document the
exact server port and seed needed to reproduce the top-right versus bottom-right configuration.

## Manual Testing Focus

Run the known right-side two-AI case and inspect Scout Car harassment in a spectator or self-play
replay. Confirm the diagnostic trace explains the selected route, Scout Cars avoid the far-right
one-tile choke as their first plan, and route memory is visible when a flank becomes occupied.

## Handoff Expectations

The handoff must summarize the diagnostics added, the fastest regression command for route
behavior, and any remaining limitations that future AI movement work should respect. It should also
include factual patch-note bullets for player-facing AI harassment changes.

## Player-Facing Outcome

AI Scout Car harassment becomes easier to debug and less likely to regress. Players should see
harassing Scout Cars use more credible flanks, avoid repeated choke loops, and react more naturally
when a flank is occupied.
