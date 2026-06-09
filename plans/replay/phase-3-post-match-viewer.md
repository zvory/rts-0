# Phase 3 - Automatic Post-Match Viewer

## Objective

When a real match ends, immediately transition connected players into replay playback from tick 0
at `2.0x` speed.

## Server Work

- Capture `ReplayArtifactV1` in `end_match` before dropping the live `Game`.
- Send the normal `gameOver` result, but do not leave players stranded on a frozen score screen. 
- Transition the room from `InGame` to `ReplayViewer` with the captured artifact. Score screen should persist until closed by clicking X or clicking anywhere that isn't the score screen panel.
- Send a replay start payload to every connected human in the room, including players eliminated
  earlier who already received an individual `gameOver`.
- Default every viewer's replay fog selection to all players' combined authoritative vision.
- Clear or overwrite pending latest-only live snapshots before sending replay start so stale live
  snapshots cannot arrive after the replay transition.
- Preserve enough lobby/rematch state so players can leave replay and return to a clean lobby.
- Keep match-history writing detached and non-blocking.

## Client Flow

- On replay start, tear down the live `Match` instance and construct replay mode.
- Keep the score result available as closable panel, but let replay playback begin
  immediately behind it.
- Provide a clear way back to lobby.

## Verification

- Integration test a two-player match ending and both clients receiving replay start at tick 0.
- Test an eliminated player is included in the replay transition if still connected.
- Test a slow client cannot receive a stale live snapshot after replay start.
- Test room can return from replay to lobby and start another match.

## Player-Facing Outcome

After the match resolves, players immediately see the match replay from the beginning at `2.0x`
speed, with all real player vision combined by default.
