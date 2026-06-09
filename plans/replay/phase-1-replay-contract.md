# Phase 1 - Replay Contract

## Objective

Define the production replay artifact and protocol contracts before changing room lifecycle. This
phase should make replay data self-describing enough to rebuild the exact initial world or reject
the replay with a clear incompatibility reason.

## Server Work

- Introduce `ReplayArtifactV1` in an appropriate shared/server crate boundary.
- Include:
  - artifact schema version
  - server build SHA
  - map name
  - map schema version
  - map content hash or stable map asset id
  - seed
  - starting steel and oil
  - starting loadout mode
  - player inits in seat order
  - duration ticks
  - command log
  - winner and final scores
- Add construction helpers that capture the artifact from a live `Game` before it is dropped.
- Add validation helpers that compare artifact build/map metadata against the running server.
- Keep legacy dev/self-play replay artifacts loadable only through the existing dev path.

## Protocol Work

- Add explicit replay metadata to `start` or a new reliable `replayStart` message.
- Add a `replayState` server message carrying shared playback state:
  - current tick
  - duration ticks
  - speed
  - paused/ended state
  - optional controller id
- Add a client replay-vision control message or field that lets a viewer request:
  - all player vision
  - one player's vision
  - a specific subset of player ids
- Keep replay vision selection per viewer in the first implementation. It should not change other
  viewers' perspective unless a later explicit shared-view feature is added.
- Update Rust and JS protocol mirrors and `docs/design/protocol.md` together.

## Verification

- Unit test artifact capture from a small deterministic match.
- Unit test validation rejects mismatched build SHA, missing map, wrong map schema, and hash
  mismatch.
- Unit test replay vision requests reject unknown player ids and empty invalid subsets.
- Unit test legacy dev replay loading remains unchanged.

## Player-Facing Outcome

No visible gameplay change yet. This phase creates the contract that makes later replay viewing
stable instead of brittle.
