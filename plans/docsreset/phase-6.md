# Phase 6 - Lobby, Protocol, Replays, and Match History

Status: done.

## Goal

Fix stale active docs for multiplayer/lobby flow, WebSocket protocol, replay and branch behavior,
match-history persistence, and deployment-facing API claims.

## Scope

- Audit lobby states, host/slot controls, faction/profile/map selection, launch payloads, room
  reset behavior, WebSocket command/event/snapshot shapes, compact protocol tables, replay loading,
  replay branching, crash/dev replay paths, match-history write/read ownership, and env-gated
  persistence.
- Fix docs that describe old fields, old commands, old replay names, old room lifecycle behavior,
  or stale match-history ownership.
- Preserve wire compatibility identifiers and compact numeric codes unless code actually changed
  them.
- Do not change protocol, lobby, replay, or persistence behavior.

## Suggested Evidence

- `docs/context/protocol.md`
- `docs/context/match-history.md`
- `docs/context/deployment.md`
- `docs/design/protocol.md`
- `docs/design/match-history.md`
- `docs/design/server-sim.md`
- `docs/design/hardening.md`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `server/src/lobby/**`
- `server/src/main.rs`
- `server/src/db.rs`
- `client/src/lobby*.js`
- `client/src/replay_*.js`
- `client/src/match_history.js`
- `tests/protocol_parity.mjs`
- `tests/client_contracts.mjs`

Useful searches:

```bash
rg -n "lobby|room|slot|map|profile|faction|WebSocket|snapshot|event|command|compact|replay|branch|match history|RTS_RECORD_MATCHES|launch" docs/design docs/context server/src client/src server/crates/protocol tests -S
```

## Verification

Run focused checks that match the final diff. Likely commands:

```bash
node scripts/check-wiki.mjs
node scripts/check-docs-health.mjs
git diff --check
```

If protocol contract docs or generated protocol references are touched, run the relevant protocol
parity/client contract check identified by the executor.

## Manual Testing Focus

Later manual smoke should cover lobby setup, map/profile selection, replay load/branch entry
points, and match-history visibility if those docs changed.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must list protocol/lobby/replay
claims fixed, compatibility identifiers intentionally preserved, verification run, and manual
flows needing later smoke.
