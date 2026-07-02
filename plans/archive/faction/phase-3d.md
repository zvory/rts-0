# Phase 3D - Replay, Branch, and Dev Lifecycle Tests

Status: Done.

## Objective

Harden non-lobby lifecycle paths so future faction content cannot enter through replay, branch,
self-play, match-history, or dev paths without an explicit source of faction truth.

## Scope

- Replay playback loads faction ids from recorded replay artifact player rows. It must not recompute
  faction ids from lobby state or current defaults.
- Replay branch staging and launch copy recorded player faction ids from the source replay seed.
  Seat claims do not alter faction ids.
- Unknown or unsupported faction ids in replay/branch/dev inputs reject cleanly unless a row is
  explicitly documented as deferred.
- Schema-1 replay artifacts without faction ids remain intentionally incompatible.
- Quickstart/debug starts default to Kriegsia unless a later phase explicitly adds a dev fixture
  path.
- Self-play and AI-driven lifecycle paths remain Kriegsia-only.
- Match-history replay uses the stored replay artifact as source of truth and rejects old or
  unsupported faction data.
- `phase2_empty_fixture` remains restricted to Rust tests and explicitly documented dev/test
  harnesses.

## Expected Touch Points

- `server/crates/sim/src/game/replay.rs`
- `server/src/lobby/dev_replay.rs`
- `server/src/lobby/room_task.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/`
- `server/crates/ai/src/selfplay/`
- `plans/faction/lifecycle-matrix.md`
- Focused Rust/protocol/server integration tests

## Verification

- Replay artifact tests prove the then-current schema preserves `kriegsia` faction ids and older
  schemas remain rejected.
- Replay branch tests prove branch seats and launches preserve recorded faction ids.
- Dev scenario/self-play tests prove starts default to Kriegsia and reject unsupported factions
  where faction input is representable.
- Match-history or replay-loading tests prove stored faction ids are source of truth.

## Manual Testing Focus

Start a normal replay and a replay branch from a current match artifact and confirm faction metadata
does not change during playback, branch staging, or branch launch.
