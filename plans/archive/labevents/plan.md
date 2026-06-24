# Lab Event Projection Plan

Lab full-world mode currently shows all entities but still attaches transient events as if the
viewer were Player 1. That makes P2-issued lab actions, such as Mortar Team fire, simulate and
impact correctly while their owner-scoped launch markers can be omitted from the operator's
snapshot. This plan fixes the underlying event-projection contract so lab-visible events match the
lab-visible world instead of adding a mortar-specific client fallback.

## Phase Summaries

Phase 1 separates snapshot-body projection from transient-event projection in the room fanout path.
Lab full-world recipients should keep the existing full-world snapshot body but receive the union of
events for all active lab players, while lab team/teams vision should continue to receive only the
selected player/team event union. The phase adds focused server tests for P2 lab mortar launch
delivery, multiple lab viewers, and unchanged normal live privacy behavior.

Phase 2 adds an externally observable regression for the lab mortar warning flow. It should drive a
real lab room through the same `issueCommandAs(P2, useAbility(mortarFire))` path the browser uses,
assert that `mortarLaunch` arrives before `mortarImpact`, and document the manual browser check for
the persistent ground warning circle. This phase should stay small and avoid turning the whole lab
tool surface into a broad integration suite.

## Phase Index

1. [Phase 1 - Server Event Projection Contract](phase-1.md)
2. [Phase 2 - Lab Mortar Regression Coverage](phase-2.md)

## Overall Constraints

- Fix the projection contract, not the symptom. Do not synthesize persistent mortar warning circles
  from local command feedback, switch the lab view player to the last command owner, or special-case
  P2 mortars.
- Keep normal fog/privacy semantics intact. Player snapshots should still receive only their own
  event bucket, live spectators should continue using the existing spectator event policy, and
  hidden enemy mortar launch data must not leak in normal live matches.
- Keep lab behavior explicit. Full-world lab vision may see the full lab event union because the
  room has intentionally granted full-world lab visibility; team and teams lab vision should only
  union events for the selected lab-visible players.
- Avoid destructive event reads for shared projections. If more than one lab recipient views the
  same union, both recipients must receive the same transient events in the same tick.
- Keep the protocol shape unchanged unless implementation proves a new wire field is necessary.
  The expected fix is server fanout/projection behavior, not a client/server message vocabulary
  change.
- If documentation changes are needed, update `docs/design/protocol.md` and the relevant context
  capsule in the same phase as the contract change.
- Use focused verification. Prefer targeted Rust room/projection tests for Phase 1 and a narrow
  Node/WebSocket or browser smoke regression for Phase 2.
- Each phase must be implemented on its own `zvorygin/` branch from `origin/main`, pushed as an
  owned PR with auto-merge armed, and waited through `scripts/wait-pr.sh <pr>` until the PR is
  merged and the head SHA is reachable from `origin/main`.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what the next agent should do, what changed, any discovered constraints, and the core manual tests
  to run. Manual testing notes should cover the lab mortar warning flow and a normal live mortar
  privacy sanity check, not an exhaustive test matrix.
