# Phase 4 - AI and Self-Play Out of the Sim Core

Status: Planned.

Goal: enforce that the simulation does not import AI while preserving live AI behavior and
self-play coverage.

## Scope

- Move `game/ai.rs`, `game/ai_core/`, and `game/ai_shared.rs` into an AI crate or AI package.
- Introduce a public sim observation/query surface sufficient for live AI:
  - map summary;
  - player summaries;
  - own entities;
  - visible enemy entities;
  - resources;
  - placement query or placement service adapter;
  - current tick.
- Move AI controller ownership out of `Game`.
- Have the room task or a match orchestration layer call AI controllers before `game.tick()` and
  enqueue emitted `SimCommand`s through the public sim API.
- Keep AI commands subject to identical validation and command logging.
- Move profile-backed self-play adapters that depend on AI profiles into the AI or tools layer.
- Keep purely sim-facing replay/determinism code in sim.

## Design Notes

The final direction should be:

```text
rts-ai -> rts-sim public API
rts-sim does not know rts-ai exists
```

This likely requires a temporary compatibility layer because `Game::new_*random_ai_profiles*`
constructors currently choose AI profiles internally. Replace those constructors with server-side
AI setup, then remove the old constructors once call sites are migrated.

AI fairness depends on observation filtering. The new observation API must preserve the current
rule: own/resource state can be authoritative, enemy state is filtered through that player's fog.

## Tests

- AI unit/profile tests.
- Live lobby AI add/remove/start integration tests.
- `cd server && RTS_FULL_AI_TESTS=1 cargo test` when AI behavior or profile-backed self-play moves.
- Replay determinism tests to verify AI-emitted command logs still replay without live AI thinking.

## Done

- `rts-sim` has no dependency on AI modules or profile lists.
- Live AI matches behave the same from the lobby and still resolve.
- AI commands remain ordinary `SimCommand`s and are recorded/replayed deterministically.
- Self-play/profile CLIs still work through the new crate boundaries.

