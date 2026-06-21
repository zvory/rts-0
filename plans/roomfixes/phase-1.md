# Phase 1 - Replay Start Capabilities

## Phase Status

- [ ] Not started.

## Objective

Fix replay viewer start/resend payloads so capabilities survive both initial replay joins and
seek-triggered `Start` resends. Add focused guardrails proving replay start payloads carry the room
policy's replay controls, diagnostics, and spectator-only shape without duplicating late-spectator
or lab-collaboration work.

Before running the roomfixes plan, confirm PR #257 and PR #258 are merged into `main`. This phase
depends on their room-policy context but must not reimplement spectator admission, live-room
spectator behavior, lab collaboration, or unrelated lobby-browser work.

## Work

- Fix the replay start payload capability loss:
  - current initial replay joins call `send_replay_start_to`, which builds
    `ReplaySession::start_payload_for`, then patches diagnostics and capabilities from
    `RoomTask`;
  - seek paths rebuild `StartPayload` directly from `ReplaySession::start_payload_for` for every
    viewer and currently patch only diagnostics;
  - `ReplaySession::start_payload_for` itself defaults `capabilities`, so any resend path that does
    not restamp capabilities loses replay controls.
- Centralize replay-viewer start payload stamping so initial joins, persisted replay joins,
  post-match replay transitions, absolute seeks, and relative seeks all use the same helper.
- Preserve replay metadata, tick, spectator identity, and diagnostic behavior while applying
  `session_policy().start_capabilities(false)` consistently for replay viewers.
- Keep capabilities server-authoritative and role-aware. Replay viewers should remain spectators;
  no active-player command, pause, or prediction capability should be advertised through this fix.
- Add focused Rust regression tests around replay start payloads:
  - direct replay join/start contains replay metadata, observer diagnostics, and replay room-time or
    replay-control capabilities expected from the active session policy;
  - absolute and relative seek resends preserve the same capabilities as the initial replay start;
  - the test should fail if a seek path only patches diagnostics.
- Add small helper tests or assertions near existing replay room/task tests rather than broad new
  harnesses.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/replay_session.rs` only if the cleanest fix moves capability stamping out of
  `ReplaySession` or adjusts its helper contract
- focused Rust tests in `server/src/lobby/room_task.rs`
- `docs/design/protocol.md` only if the implementation changes documented `start.capabilities`
  semantics, not just restores existing replay policy on resend

## Implementation Checklist

- [ ] Confirm PR #257 and PR #258 are merged before starting implementation.
- [ ] Identify every replay `ServerMessage::Start` send and resend path.
- [ ] Add or reuse one room-task helper that stamps replay start payload diagnostics and
      capabilities together.
- [ ] Ensure absolute seek and relative seek resend paths use that helper.
- [ ] Ensure initial replay joins and replay-room transitions keep using the same stamping logic.
- [ ] Preserve `replay: Some(...)`, `spectator: true`, viewer `player_id`, and authoritative tick.
- [ ] Add focused Rust regression coverage for initial replay start and seek-triggered resends.
- [ ] Avoid spectator admission, lab-collab, lobby-browser, or unrelated room-policy changes.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase file done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server replay_start_capabilities -- --nocapture`
- If the final test names use a different filter, run the exact added replay start/resend test names
  instead of counting a zero-test filter as passed.
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay_join_and_seek_emit_authoritative_analysis -- --nocapture`
- `git diff --check`

Do not run broad bundles by default. Rely on the PR `./tests/run-all.sh` gate for full-suite
coverage unless the implementation touches a wider contract.

## Manual Test Focus

Open a saved or persisted replay, confirm the replay controls advertised by the initial start
payload are available, then seek backward and seek to an absolute tick. After each seek, confirm the
client still has the replay controls and observer diagnostics it had on initial join, and that no
active-player controls appear.

## Handoff Expectations

Summarize the shared replay start helper shape, the exact capability fields covered by the
regression tests, and which replay start paths now call the helper. Include the focused verification
commands and manual replay seek results, and call out any remaining roomfixes work without pulling
spectator or lab-collab scope into this phase.
