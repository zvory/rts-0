# Live Spectator Join Plan

## Purpose

Let anyone spectate a normal match that is already in progress from the lobby browser. Active
mid-match joins must remain rejected, but spectator joins should attach to the existing live match as
read-only observers without changing match seats, win conditions, command authority, or fog rules.
When a spectator joins mid-match, existing match recipients should see an info notice:
`<name> has joined the match as a spectator`, with `Commander` used when the join name is blank or
otherwise unavailable.

## Current Baseline

- `GET /api/lobbies` already includes normal live matches as browser rows with
  `joinState: "inGame"`, but the client currently treats those rows as non-joinable.
- `RoomTask::on_join` currently rejects normal-room joins after the match starts, regardless of the
  requested spectator flag.
- Live spectator projection, read-only start payloads, command blocking, observer analysis, and
  spectator audio suppression already exist for spectators who joined before match start.
- `Event::Notice` already surfaces toast-style messages through snapshots; adding the spectator join
  notice should reuse that event shape unless implementation evidence proves a small control-plane
  message is necessary.

## Overall Constraints

- Keep the server authoritative. The lobby browser can present the action, but `RoomTask` must
  decide whether a late join is accepted.
- Accept only `join { spectator: true }` after a normal match has started. Active late joins,
  countdown joins, replay-room confirmation rules, branch rooms, lab rooms, and dev-watch rooms keep
  their existing mode-specific behavior unless a phase explicitly scopes otherwise.
- Do not add a modal, confirmation dialog, or extra identity prompt. Use the existing join name from
  the lobby flow and fall back to `Commander` when the name is blank or hard to recover.
- Keep late spectators out of `PlayerInit`, active seat lists, team assignments, command issuers,
  pause authority, elimination, scores as players, and match-player counts.
- Preserve fog safety. Late spectators should receive the same union-fog projection current live
  spectators receive, and no active player should receive enemy data they cannot already see.
- Keep the lobby browser's HTTP polling/preflight model. Do not add a WebSocket lobby-list push
  protocol for this feature.
- Keep compact snapshot schema stable unless implementation evidence shows that a compact event or
  snapshot change is unavoidable.
- If spectator join notices use snapshot `Event::Notice`, target the event to active players and
  already-connected spectators. Do not accidentally notify only active seats through per-player sim
  events, and do not include the newly joined spectator unless an explicit product decision changes
  that requirement.
- If a live match is paused when a spectator joins, do not resume simulation just to flush the
  notice. Either queue the notice for the next emitted snapshot or add a narrowly-scoped reliable
  notice path if the phase proves immediate paused-match delivery is required.
- Update protocol, server-sim, and client-ui docs when join semantics, event routing, or browser
  behavior change.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is reachable
  from `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Late Spectator Admission](phase-1.md)

Make the lobby browser's in-match rows intentionally spectatable instead of disabled. The server
should accept only spectator mid-match joins, send the same read-only live start payload shape used
for lobby-time spectators, and keep active late joins plus countdown joins rejected. The result
should be that any user can click an in-progress row, enter read-only spectator mode, and receive
existing union-fog snapshots without being seated in the match.

### [Phase 2 - Join Notice And Lifecycle Polish](phase-2.md)

Add the player-facing notice when someone spectates a live match after it has already started. The
server should render `<name> has joined the match as a spectator`, fall back to `Commander` when the
name is blank or unusable, and deliver the notice to every active player and already-connected
spectator without opening any modal. This phase should also close lifecycle and test gaps around
stale browser rows, paused matches, disconnects, and documentation so the feature is shippable.

## Phase Index

1. [Phase 1 - Late Spectator Admission](phase-1.md)
2. [Phase 2 - Join Notice And Lifecycle Polish](phase-2.md)

## Non-Goals

- Do not allow late active players to join a running match.
- Do not let late spectators claim seats, issue commands, pause/unpause, give up as players, affect
  match history participant lists, or change victory resolution.
- Do not hide in-progress rows from the lobby browser; the feature depends on those rows remaining
  visible.
- Do not add spectator passwords, privacy controls, invitations, or match lock settings.
- Do not add a spectator count cap unless load testing or production evidence justifies a separate
  product decision.
- Do not implement replay-room, lab-room, branch-live, or dev-watch admission changes beyond keeping
  their current behavior intact.
- Do not add a new browser build step or frontend framework.

## Suggested Execution

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`.

```bash
scripts/phase-runner.sh --plan spectator phase-1 --pr --wait
scripts/phase-runner.sh --plan spectator phase-2 --pr --wait
```
