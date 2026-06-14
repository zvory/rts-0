# Phase 2 - Strategic Goal Skeleton and Debug Trace

Status: Implemented.

## Objective

Introduce the light manager architecture without changing live AI behavior. The new skeleton should
make the AI's current strategic phase, active goals, blockers, reservations, and emitted intents
visible enough to debug self-play failures.

## Scope

- Define explicit goal and blocker types for economy, supply, expansion, tech, production, local
  defense, frontal attack, and harassment.
- Add a manager output structure that records:
  - selected goal or skipped goal
  - blockers and prerequisites
  - high-level intents requested
  - commands emitted through `AiActionContext`
  - important budget and reservation decisions
- Wrap the current `decide_profile` ordering with manager-like functions or adapters while keeping
  command output behavior-equivalent.
- Add deterministic trace formatting for tests and self-play artifacts.
- Keep the live profile pool and default profile unchanged.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/policies.rs`
- new focused files under `server/crates/ai/src/ai_core/decision/` if needed
- `server/crates/ai/src/ai_core/actions.rs`
- `server/crates/ai/src/selfplay/replay.rs` or diagnostics artifact code if traces are persisted
- `docs/design/ai.md`

## Verification

- Add focused pure tests that compare old and new skeleton output for representative observations.
- Assert deterministic trace ordering and stable blocker labels.
- Run:

```bash
cd server && cargo test -p rts-ai
```

- Run a short baseline matchup and confirm command counts, first attack timing, and final counts are
  materially unchanged unless a documented trace-only side effect explains the difference.

## Manual Testing Focus

Inspect a self-play or matchup artifact and confirm the AI trace explains why it chose to build,
train, gather, stage, attack, or skip a goal. Confirm live lobby AI still plays like the previous
baseline.

## Handoff Expectations

The handoff must describe the new goal/blocker types, where traces are emitted, and how later
phases should add manager behavior without bypassing `AiActionContext`. It must call out any
remaining behavior-equivalence differences versus the baseline.

## Player-Facing Outcome

No intended gameplay change. The AI becomes easier to inspect and evolve.
