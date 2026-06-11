# Phase 8B - Branch Staging Runtime

## Objective

Add a server-side branch staging phase where moved replay viewers can claim original seats before
the resumed match starts.

## Server Work

- Add a branch staging room phase, separate from normal `Lobby`, `InGame`, and `ReplayViewer`.
- Store frozen branch seed state:
  - source replay metadata
  - branch tick
  - rebuilt `Game` at the branch tick
  - original player seats in order
  - seat claim state
- Treat all moved replay viewers as branch-room occupants.
- Allow each human occupant to claim at most one original seat.
- Allow seats to be released/reclaimed while staging is active.
- Require every original active seat to be claimed before the branch can launch.
- Keep spectators as spectators in the branch room if they do not claim a seat.
- Do not allow adding AI, quickstart changes, map changes, or normal lobby player reshuffling in
  branch staging.
- Make the first branch-room occupant the host for launch purposes, with normal host fallback on
  leave.
- If the branch room empties, drop the frozen branch state and reset/close the room safely.

## Protocol Work

- Add branch staging state to server messages:
  - room id
  - source replay tick
  - host id
  - seats with original player id/name/color and claimant id/name if claimed
  - viewer/spectator occupants
  - can start
- Add client messages:
  - claim branch seat
  - release branch seat
  - start branch
- Keep staging messages distinct from normal lobby messages unless reusing lobby rendering is
  deliberately chosen and documented.

## Verification

- Unit test seat claims are exclusive.
- Unit test one occupant cannot claim multiple seats.
- Unit test all original seats must be claimed before `canStart`.
- Unit test leaving releases claimed seats and reassigns host.
- Unit test branch staging rejects normal lobby-only controls.
- Regression test empty branch rooms drop frozen state and do not poison the room name.

## Player-Facing Outcome

Viewers arrive in a branch staging room, claim original seats, and can see when the branch is ready
to launch.
