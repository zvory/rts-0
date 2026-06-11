# Phase 8E - Branch Hardening And Verification

## Objective

Add deeper determinism, failure, lifecycle, and regression coverage after the branch flow works end
to end.

## Determinism Tests

- Replay reconstruction determinism:
  - run a deterministic match to tick N
  - capture replay artifact
  - rebuild replay to tick N
  - compare authoritative snapshots/events against the live state at tick N
- AI command-log determinism:
  - run two deterministic AI profiles for one minute
  - capture the command log
  - rebuild to 30 seconds
  - replay the remaining recorded commands from the rebuilt state
  - verify the final snapshots and event stream match the original run
  - do not treat this as AI resumption support
- Branch divergence test:
  - branch from tick N
  - issue a new human command after launch
  - verify the branch command log diverges from the source replay only after tick N

## Failure Tests

- Failed branch request leaves source replay playback intact.
- Failed staging launch leaves branch staging intact.
- Failed countdown validation returns to staging instead of dropping the room.
- A viewer leaving during countdown releases their seat and aborts launch cleanly.
- Malformed branch seat messages cannot panic the room task.
- Repeated branch requests are rate-limited or otherwise bounded.

## Fog And Security Tests

- Branch live snapshots ignore replay viewer fog selections.
- Branch live events are fog-gated like ordinary live events.
- Branch spectators cannot issue commands.
- Claimed players cannot command another claimed seat.
- Seat mapping cannot target unknown, duplicate, or unclaimed seats.

## Lifecycle Tests

- Source replay artifact remains immutable after branch creation.
- All replay viewers are moved to branch staging.
- Empty branch rooms clean up frozen state.
- Branch rooms cannot be joined as ordinary lobby rooms mid-setup unless explicitly allowed.
- Returning/leaving after branch game-over does not poison the original replay room.

## Performance And Observability

- Log branch creation tick, source replay metadata, branch room id, viewer count, and rebuild time.
- Bound expensive rebuild work using the replay keyframe path where possible.
- Add metrics/logs for failed branch requests and launch aborts.

## Player-Facing Outcome

Resume-from-replay is robust enough for normal use: failed operations keep rooms usable, fog remains
authoritative, and deterministic branch behavior is covered by tests.
