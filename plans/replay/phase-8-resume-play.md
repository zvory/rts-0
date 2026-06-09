# Phase 8 - Resume Play From Replay

## Objective

Allow players to branch a replay at a chosen tick and resume interactive play from that point. This
is intentionally last because it turns replay from read-only analysis into authoritative match
creation and introduces game-design, fairness, and persistence questions.

## Product Shape

- A viewer can choose a replay tick and request "resume from here".
- The server creates a new live match initialized from the replay state at that tick.
- Players can select any seat, all seats must be occupied by players or left idle, no AI resumption.
- The resumed match is a new match, not a mutation of the original replay.
- The new match should record its own match history and, eventually, its own replay artifact.

## Server Work

- Add a way to reconstruct a `Game` up to an exact replay tick and then promote it into a normal
  `Phase::InGame`.
- Define whether resumed games keep original player ids or allocate new connection ids mapped to
  original seats. Prefer an explicit seat mapping so reconnects and invited players are clear.
- Clear replay-only state when promotion happens:
  - replay driver cursor
  - replay speed
  - per-viewer fog selections
  - replay overlays/state messages
- Reinitialize live match fields:
  - match start wall clock
  - participants
  - human and AI counts
  - net health counters
- Require compatibility validation before branching from persisted replays.
- Decide persistence semantics for branched matches. Recommended first pass: mark the match as a
  normal new match, with optional future metadata pointing back to the source replay id and tick.

## Client Work

- Add a "Resume from here" action guarded by a confirmation flow.
- Let the initiating viewer configure seat ownership before launching, or defer to a lobby-like
  staging screen.
- Tear down replay viewer and construct normal `Match` on live start.
- Make it clear that resumed play is a new branch, not changing the original result.

## Hard Questions To Resolve Before Implementation

- Can one player resume alone as a sandbox, or must all original seats be filled? Answer: all original seats must be filled.
- Are resumed matches eligible for public match history? Answer: No. 
- Are players allowed to resume from opponent-only fog perspectives they could not see live? Answer: Yes.
- Should ranked/competitive modes ever allow resume-from-replay, or is this only a learning tool? Defer, ignore, no need to asnwer now. 

## Verification

- Determinism test that replay fast-forward to tick N produces the same state as live match tick N.
- Integration test replay -> resumed live match -> commands apply normally.
- Test resumed live match uses fog-filtered player snapshots, not replay viewer fog selections.
- Test resumed match can end and record independently from the source replay.
- Regression test failed resume requests leave the replay session intact.

## Player-Facing Outcome

Players can stop at a mistake, branch the game from that exact moment, and try a different response.
This turns replay from review into practice.
