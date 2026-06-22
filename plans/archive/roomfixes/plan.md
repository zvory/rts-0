# Room Policy Fixes Plan

## Purpose

Close the room-policy refactor gaps found in the architecture review without racing the active
spectator and collaborative-lab phase runners. The room shell is now mostly unified around
`SessionPolicy`, room capabilities, projection policy, and shared `RoomTask` lifecycle code, but a
few old product-specific paths still bypass those policy seams and create real behavior bugs. This
plan fixes the concrete regressions first, then tightens the start-payload and room-time boundaries
so future room modes do not have to rediscover the same special cases.

## Current Situation

- `plans/spectator` Phase 1 is in flight as PR #257, `Allow late spectator joins`. It touches
  `server/src/lobby/room_task.rs`, `server/src/lobby/session_policy.rs`, spectator docs, and lobby
  browser tests, so this plan must not start join-policy work until that PR has merged and the head
  SHA is reachable from `origin/main`.
- `plans/lab/debug-collab` Phase 1 has merged, and Phase 2 is in flight as PR #258,
  `Enable collaborative lab client controls`. This plan does not duplicate the lab HUD, input, or
  minimap command-surface work owned by that plan; it starts from the result after Phase 2 merges.
- The architecture review found two server correctness bugs that are not owned by those active
  plans: replay seek resends a `start` payload without policy-derived capabilities, and branch-live
  attach is routed through branch-staging join code that can demote a live branch match back to
  staging.
- The review also found lifecycle/design debt: internal branch room identity can decay into a public
  normal room, lab auto-start bypasses the drain gate, dev/replay room-time controls still have
  product-specific client assumptions, and `StartPayloadPolicy` is classified but not yet the code
  path that actually builds start payloads.

## Overall Constraints

- Start every implementation phase from fresh `origin/main`. Do not start Phase 1 until PR #257 and
  PR #258 are merged, unless the user explicitly redirects the active phase runners.
- Preserve unrelated dirty state, especially `playtest_notes.md`; implementation phases should use
  isolated `/tmp/rts-worktrees` worktrees and `zvorygin/` branches.
- Keep `plans/spectator` and `plans/lab/debug-collab` ownership intact. If those plans are still
  open, treat their files and active touch points as unavailable unless the user explicitly asks to
  merge the work into `plans/roomfixes`.
- Prefer concrete bug fixes before architectural cleanup. Do not start the start-payload builder
  refactor until the replay seek, branch-room, lab drain, and room-time client fixes have landed.
- Keep policy vocabulary executable. When a phase introduces or relies on a `SessionPolicy` axis, it
  should either consume that axis in behavior or state clearly why it is documentation-only.
- Keep protocol mirrors together if a phase changes `StartPayload`, `RoomCapabilities`,
  `ServerMessage`, `ClientMessage`, or their docs.
- Preserve fog safety. Changes to replay, branch, lab, spectator, or projection behavior must not
  send a player entity or position data they cannot see.
- Preserve room drain semantics. Any room mode that starts or counts an authoritative live session
  must make an explicit drain decision instead of relying on a hidden mode exception.
- Use focused verification for each phase, then rely on the PR `./tests/run-all.sh` gate for the
  full suite.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- After each phase, the implementing agent must provide a handoff message with exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Replay Start Capabilities](phase-1.md)

Make every replay `start` resend use the same policy-derived capability and diagnostic construction
as initial replay joins. Replay seek, absolute seek, post-match replay attachment, and dedicated
replay-room starts should preserve room-time controls, replay vision, diagnostics, and any branch
affordance metadata instead of rebuilding partial payloads. The phase should add focused coverage so
future replay restart paths cannot silently drop `StartPayload.capabilities` again.

### [Phase 2 - Branch Room Admission And Identity](phase-2.md)

Separate branch-live attach from branch-staging join so a late join cannot replace an active branch
match with staging state. Keep internal replay-branch room identity private even after empty-room
cleanup, or expire those internal rooms instead of mutating them into public normal lobbies. The
phase should build on the merged spectator join shape and prove normal late spectators, branch
staging, and branch-live rooms all keep distinct admission behavior.

### [Phase 3 - Lab Launch Drain Policy](phase-3.md)

Put lab auto-start behind an explicit drain policy so an existing lab room cannot launch a new
authoritative live session during deploy drain by joining the room. Decide in code whether labs are
drain-tracked authoritative sessions or intentionally non-drain-tracked tools, and make
`SessionPolicy` or a small helper express that decision. The phase should keep collaborative lab
client behavior from `plans/lab/debug-collab` intact while covering drain, empty-room reset, and lab
start metadata.

### [Phase 4 - Room-Time Client Capability Cleanup](phase-4.md)

Remove the remaining stale product assumptions in shared room-time controls. The dev scenario Step
button should use the neutral room-time data attribute, speed/pause/step/seek/timeline controls
should be rendered or activated from their matching capability bits, and replay-only controls should
not masquerade as generic room-time affordances. The phase should stay client-focused unless a
small server capability addition is required for branch eligibility.

### [Phase 5 - Start Payload Policy Builder](phase-5.md)

Turn `StartPayloadPolicy` from a classification into the construction seam for live, spectator,
branch-live, lab, dev-watch, replay, and replay-restart starts. Introduce a small launch/start
payload builder that consumes `SessionPolicy`, recipient role, projection diagnostics, and
state-source metadata, while keeping the existing `Game` API seam stable. The phase should remove
duplicated payload mutation paths without broad room-task rewrites.

## Phase Index

1. [Phase 1 - Replay Start Capabilities](phase-1.md)
2. [Phase 2 - Branch Room Admission And Identity](phase-2.md)
3. [Phase 3 - Lab Launch Drain Policy](phase-3.md)
4. [Phase 4 - Room-Time Client Capability Cleanup](phase-4.md)
5. [Phase 5 - Start Payload Policy Builder](phase-5.md)

## Suggested Execution

Do not run this plan until PR #257 and PR #258 have merged unless the user explicitly pauses those
phase runners. After both are merged and `origin/main` is refreshed, run one phase at a time and
wait for each PR to merge before starting the next phase.

```bash
scripts/phase-runner.sh --plan roomfixes phase-1 phase-2 phase-3 phase-4 phase-5 --pr --wait
```

If a phase-runner pass discovers that `plans/spectator` or `plans/lab/debug-collab` has already
fixed one of these items, it should update the relevant phase file with merged-PR evidence and
either narrow the phase or mark it complete in that phase's implementation commit.
