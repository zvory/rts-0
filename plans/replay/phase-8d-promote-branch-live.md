# Phase 8D - Promote Branch To Live Match

## Objective

Promote a fully claimed branch staging room into a normal live match initialized from the frozen
replay state.

## Server Work

- Reuse the existing lobby countdown behavior before promotion, including the same countdown words
  and timing.
- At countdown completion, validate that all original seats are still claimed.
- Promote the frozen branch `Game` into `Phase::InGame`.
- Use explicit connection-to-seat mapping for live commands and snapshots:
  - connected client id maps to original replay player id
  - commands are enqueued for the mapped original player id
  - snapshots for claimed seats use `snapshot_for(original_player_id)`
  - branch spectators use spectator union fog for the active branch players
- Send normal live `start` payloads with each claimed player's mapped seat id.
- Clear replay-only and staging-only state:
  - replay cursor
  - replay speed
  - per-viewer replay fog selections
  - replay/staging overlays
  - pending latest-only snapshots
- Reinitialize live match fields:
  - match start wall clock
  - participant names from claimed seats
  - human count
  - match player count
  - net health counters
  - outcome state
- Keep `ai_controllers` empty for branch matches.
- Mark branch matches as non-public/non-recorded for the first implementation.
- Ensure give-up, elimination, game-over, and post-match replay transition still work for branch
  matches, except public match-history recording remains disabled.

## Client Work

- On branch live start, tear down branch staging and construct normal `Match`.
- Make command input, HUD, selection, minimap, and give-up behave like an ordinary live match.
- Preserve spectator behavior for branch occupants who did not claim a seat.
- Handle countdown messages using the same UI path as normal lobby start.

## Verification

- Integration test replay -> branch staging -> countdown -> live start.
- Test commands from a claimed connection apply to the mapped original replay seat.
- Test claimed players receive fog-filtered snapshots for their mapped seat, not replay viewer fog.
- Test branch spectators receive spectator snapshots and cannot issue commands.
- Test stale replay/staging snapshots are cleared before live start.
- Test give-up can end a branch match.
- Test branch match does not write public match history.

## Player-Facing Outcome

Once every original seat is claimed, the branch plays the normal countdown and becomes a live match
from the replay tick.
