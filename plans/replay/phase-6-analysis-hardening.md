# Phase 6 - Scrubbing, Overlays, and Hardening

## Objective

Evolve the replay viewer into an analysis tool while keeping replay correctness and room safety
strong.

## Analysis Features

- Timeline scrubber with current tick and duration.
- Pause and single-step controls.
- Optional overlays:
  - player economy
  - army value
  - production queues
  - supply
  - combat events
  - vision coverage
- Camera bookmarks for major events.
- Polished per-player vision filters:
  - all players
  - one player
  - selected subset
  - quick compare between two players' perspectives

## Server Hardening

- Bound seek frequency and expensive fast-forward work.
- Consider checkpoint snapshots every N ticks for long replays if seeking becomes too expensive.
- Keep replay room memory bounded.
- Add observability around replay rebuild time, seek time, and viewer count.
- Keep replay playback independent from live match tick health.

## Compatibility Policy

- Same build SHA is required initially.
- If replay compatibility becomes intentionally cross-version, introduce explicit artifact
  migrations and deterministic replay compatibility tests before relaxing the SHA gate.
- Map hash must match exactly unless the artifact embeds the full map asset.

## Verification

- Long replay seek stress test.
- Replay determinism test across representative command types.
- Load test multiple viewers in one replay session.
- Regression test rapid fog-perspective changes do not leak hidden entities or stall replay ticks.
- Regression test malformed replay artifacts and control messages cannot panic room tasks.

## Player-Facing Outcome

The replay viewer becomes a practical learning tool: players can pause, scrub, inspect economy and
vision, and quickly understand what changed the match outcome.
