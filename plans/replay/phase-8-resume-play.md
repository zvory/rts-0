# Phase 8 - Resume Play From Replay

## Objective

Allow players to branch a replay at a chosen tick and resume interactive play from that point.
This is split into smaller implementation phases because resume turns read-only replay playback
into authoritative match creation.

## Product Decisions

- Any replay viewer may request a practice branch from the replay's current server tick.
- The branch is created as a new room, not by mutating the source replay room.
- All current replay viewers are moved to the branch room.
- The branch room has a staging screen where viewers claim original replay seats.
- All original non-spectator seats must be claimed by humans before launch.
- No AI resumption in the first implementation.
- Branched matches are not eligible for public match history in the first implementation.
- Branch replay recording can be added later, but should be ignored for the first implementation.
- Launching the branch uses the same countdown behavior as normal lobby match start.

## Subphases

1. [Phase 8A - Branch Room Contract](phase-8a-branch-room-contract.md)
2. [Phase 8B - Branch Staging Runtime](phase-8b-branch-staging-runtime.md)
3. [Phase 8C - Client Branch Staging UX](phase-8c-client-branch-staging-ux.md)
4. [Phase 8D - Promote Branch To Live Match](phase-8d-promote-branch-live.md)
5. [Phase 8E - Hardening And Verification](phase-8e-branch-hardening-verification.md)

## Player-Facing Outcome

Players can stop at a mistake, create a practice branch from that exact moment, claim seats, wait
through the normal match countdown, and play forward from the replay state.
