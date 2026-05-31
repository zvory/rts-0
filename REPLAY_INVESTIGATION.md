# Replay Investigation

Date: 2026-05-31

This note records only confirmed facts relevant to the replay viewing bug where
player 2 appears idle in the dev self-play replay viewer.

Before opening any replay, stop any existing server on `:8080`, start a fresh
server, and then open the replay URL. The stale-server hypothesis in this note
assumes that rule.

## Regenerated replay artifacts

Local replay artifacts were deleted from:

- `server/target/selfplay-artifacts`
- `server/target/selfplay-failures`

After clearing them, a fresh replay artifact was generated:

- `server/target/selfplay-artifacts/real_ai_vs_real_ai_1780236634155/replay.json`

This confirms the investigation is not reading an older local replay directory.

## What the replay command log confirms

From `real_ai_vs_real_ai_1780236634155/replay.json`:

- Player 2 emits commands from the beginning of the game.
- Player 2's first commands occur at tick `7`.
- At tick `7`, player 2 emits:
  - `train(worker)` on building `26`
  - four separate `gather` commands
- Player 1 emits its corresponding first commands at tick `8`.

These facts alone rule out "player 2 never acts" in the recorded command log.

## What the replay proves about game state

The replay command log and later commands confirm that player 2's commands are
affecting authoritative state:

- At tick `133`, player 2 orders worker `51` to gather.
- Worker `51` is not a starting worker id, so at least one earlier worker train
  must have completed in authoritative state.
- At tick `367`, player 2 emits `build(depot)`.
- At tick `448`, player 2 emits `build(barracks)`.
- Later, player 2 trains `rifleman` from building ids `63`, `69`, `102`, and `116`.
- Those later `train(rifleman)` commands confirm that multiple barracks exist in
  replayed state.
- Later, player 2 emits repeated `attackMove` commands with growing wave sizes.

These facts confirm that player 2 is not inert in the replayed simulation.

## What the replay viewer actually does

The dev replay viewer is not a static log viewer.

Confirmed from `server/src/lobby.rs`:

- Replay mode loads the artifact with `load_replay_artifact(...)`.
- It creates a fresh replay game with `Game::new_for_replay(...)`.
- It constructs a `ReplayDriver` from the artifact.
- On each dev-selfplay tick, the driver enqueues recorded commands into the fresh game.
- The room then runs `game.tick()` and broadcasts snapshots from that replayed game.

Confirmed from `server/src/game/replay.rs`:

- `replay_commands(...)` also replays the recorded command log through a fresh
  `Game::new_for_replay(...)`.

## Determinism fact

The codebase already contains a replay-equivalence assertion:

- `assert_replay_matches_live(...)` compares:
  - tick count
  - emitted events
  - final snapshots

No confirmed evidence currently shows the replay artifact itself is stale or that
the replay driver is skipping player 2's recorded commands.

## Confirmed conclusion

- The replay artifact is freshly regenerated after deleting local replay output.
- The replay command log shows player 2 issuing commands from the start of the game.
- The replay command log also shows later player-2 workers, buildings, unit production,
  and attack orders, which means those commands are affecting replayed game state.
- If the replay viewer still appears to show player 2 as idle, the bug is in the
  replay viewing path or client-visible presentation, not in whether player 2 exists
  in the recorded command log.

## Commit cb49460

Commit checked out:

- `cb49460` (`Balance tanks as hard counter to massed MGs`)

Fresh replay generated after deleting local replay directories, then renamed to preserve it:

- `server/target/selfplay-artifacts/real_ai_vs_real_ai_cb49460_DONOTDELETE_1780237843063/replay.json`

Observed result in the replay viewer:

- The bug does not appear on this replay.
- We do not see escalating wave behavior.
- Wave sizes look the same as usual rather than increasing over time.

## Commit 078bf86

Commit checked out:

- `078bf86` (`Merge zvorygin/escalating-waves: escalating AI wave sizes`)

Fresh replay generated after deleting non-preserved local replay directories, then renamed to preserve it:

- `server/target/selfplay-artifacts/real_ai_vs_real_ai_078bf86_DONOTDELETE_1780238100195/replay.json`

Observed result in the replay viewer:

- The bug does not appear on this replay.
- We do not see escalating wave behavior.
- Each wave is still size four.

## Commit 86f62ae

Commit checked out:

- `86f62ae` (`Fix replay artifact recovery`)

Fresh replay generated after deleting non-preserved local replay directories, then renamed to preserve it:

- `server/target/selfplay-artifacts/real_ai_vs_real_ai_86f62ae_DONOTDELETE_1780238319362/replay.json`

Observed result in the replay viewer:

- The bug does not appear on this replay.
- We do not see escalating wave behavior.

Current hypothesis:

- A stale or otherwise bugged server process may have been causing replay viewing failures even when the replay files themselves were fine.
- This is not yet proven, but it is consistent with the fact that clean per-commit server restarts have not reproduced the viewing bug.
