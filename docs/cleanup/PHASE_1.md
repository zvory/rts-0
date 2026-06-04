# Phase 1 - Self-Play Harness Decomposition

Goal: split `server/src/game/selfplay.rs` into harness components while preserving deterministic
API-driven self-play.

## Target Components

- `selfplay/mod.rs`: public test/dev entry points and re-exports.
- `selfplay/live.rs`: `LiveSelfPlay` and live driver integration.
- `selfplay/replay.rs`: replay artifact loading, serialization, replay outcome comparison.
- `selfplay/scripts.rs`: `ScriptedPlayer`, profile-backed script, worker-rush script, mine-only
  script, and command generation helpers.
- `selfplay/player_view.rs`: `PlayerView` and snapshot query helpers used by scripts.
- `selfplay/pending_build.rs`: pending build tracking and failed-spot handling.
- `selfplay/milestones.rs`: milestone capture, combat goals, player goals, and assertion helpers.
- `selfplay/validation.rs`: snapshot/resource sanity checks and known-kind validation.
- `selfplay/tests.rs` or focused inline test modules: keep scenario tests discoverable.

## Design Notes

This file is large but mostly test/dev support, so it is the safest proving ground for the cleanup
pattern. Prefer mechanical extraction first. Keep scripts driving the public `Game` API through
`enqueue`, `tick`, and `snapshot_for`; do not let the harness reach into service internals.

Replay artifact structs should remain serde-compatible. If a struct moves modules, preserve field
names, defaults, and any legacy replay behavior.

## Tests

- Run `cargo test` in `server/`.
- Run targeted self-play tests if they are available as named test filters.
- If a self-play failure is not immediately obvious, follow the existing replay-inspection flow from
  `CLAUDE.md`.

## Done

- `selfplay.rs` becomes a small module root or is replaced by `selfplay/mod.rs`.
- Script logic, replay logic, milestone logic, and validation are separated.
- Existing self-play behavior and replay compatibility are unchanged.

