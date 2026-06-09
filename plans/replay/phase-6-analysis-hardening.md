# Phase 6 - Scrubbing, Overlays, and Hardening

## Objective

Evolve the replay viewer into an analysis tool while keeping replay correctness and room safety
strong.

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
