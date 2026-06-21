# Phase 5 - Start Payload Policy Builder

## Phase Status

- [ ] Not started.

## Objective

Turn `StartPayloadPolicy` from a classification field into the construction seam for every room
start payload after PR #257, PR #258, and roomfixes Phases 1 through 4 have merged. The new
lobby-owned builder/helper should consume `SessionPolicy`, recipient role, projection diagnostics,
and state/source metadata so live, spectator, branch-live, lab, dev-watch, replay join, and replay
restart/seek paths stamp the same recipient-scoped capabilities and diagnostics.

This phase is a narrow cleanup around start payload assembly. Preserve the `Game::start_payload()`
API as the source of static simulation start data, and do not move sim internals into lobby code.

## Work

- Replace duplicated payload stamping with one small builder/helper in the lobby layer:
  - normal live starts currently build a `Game::start_payload()` base, then stamp player id,
    spectator flag, prediction metadata, capabilities, and diagnostics through `launch.rs`;
  - branch-live starts use the same helper but add branch seat aliases and pending-snapshot
    clearing;
  - lab starts and later lab joins patch lab metadata, spectator identity, capabilities, and
    diagnostics separately;
  - dev-watch starts patch the dev view player id, disabled prediction, capabilities, and full-world
    diagnostics separately;
  - replay joins and replay seek/restart paths build from `ReplaySession::start_payload_for` and
    can drift from policy stamping, which already allowed seek resends to drop capabilities.
- Make `StartPayloadPolicy` select the builder path instead of remaining a passive description.
  The builder should take explicit inputs rather than reading broad `RoomTask` state directly:
  `SessionPolicy`, `RecipientRole`, base simulation or replay payload/source metadata,
  projection-derived diagnostics, recipient identity, prediction eligibility, pending-snapshot
  behavior, and optional lab/replay metadata.
- Keep replay metadata sourced from `ReplaySession` and lab metadata sourced from the room-owned lab
  session, but route final `StartPayload` stamping through the same policy builder.
- Preserve Phase 4 capability behavior. If Phase 4 added a small capability bit, integrate its
  population here through `SessionPolicy::start_capabilities` or the builder input; do not introduce
  unrelated protocol shape changes.
- Keep protocol shape unchanged unless Phase 4 introduced a capability bit that still needs server
  integration. If any `StartPayload`, `RoomCapabilities`, or diagnostic field shape changes, update
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`,
  `docs/design/protocol.md`, and protocol parity together.
- Update `docs/design/server-sim.md` if the ownership text for `launch.rs`, `replay_session.rs`, or
  `SessionPolicy` changes from "stamping in several places" to the new start-payload builder seam.
- Do not rewrite `RoomTask`, change admission policy, change projection/fog rules, change replay
  playback, or move `Game` internals out of `Game`.

## Expected Touch Points

- `server/src/lobby/session_policy.rs`
- `server/src/lobby/launch.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/replay_session.rs` only to narrow replay start-payload responsibility or expose
  source metadata cleanly
- `server/src/lobby/projection.rs` only if builder inputs need a small recipient-diagnostics type
- focused Rust tests in `server/src/lobby/room_task.rs` and/or `server/src/lobby/session_policy.rs`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`, protocol mirrors, and `tests/protocol_parity.mjs` only if Phase 4 left
  a required capability-bit integration or this phase changes protocol shape

## Implementation Checklist

- [ ] Confirm PR #257 and PR #258 are merged before starting implementation.
- [ ] Confirm Phases 1, 2, 3, and 4 have merged and start from fresh `origin/main`.
- [ ] Inventory every `ServerMessage::Start` send/resend path: normal live player, live spectator,
      branch live, lab initial/later join, dev watch, replay join, replay restart, and replay seek.
- [ ] Add a lobby-owned `StartPayloadPolicy` builder/helper with explicit inputs for policy,
      recipient role, payload source metadata, projection diagnostics, prediction, lab metadata,
      replay metadata, and pending-snapshot behavior.
- [ ] Route normal live launch recipients through the builder without changing active player or
      spectator semantics.
- [ ] Route branch-live launch recipients through the builder while preserving original-seat aliases
      and pending-snapshot clearing.
- [ ] Route lab initial starts and later collaborator starts through the builder while preserving lab
      metadata, operator/collaborator role data, disabled prediction, and full-world projection.
- [ ] Route dev-watch starts through the builder while preserving the dev view player id, disabled
      prediction, room-time capabilities, and diagnostic movement paths.
- [ ] Route replay joins and replay restart/seek resends through the builder so capabilities,
      diagnostics, replay metadata, spectator identity, and authoritative tick stay consistent.
- [ ] Keep `Game::start_payload()` as the static simulation start-data source and keep replay/lab
      source metadata owned by their current lobby helpers.
- [ ] Integrate any Phase 4 capability-bit addition, if present, without adding broader protocol
      changes.
- [ ] Add focused regression coverage for all start-payload classes listed in verification.
- [ ] Update design docs only for the changed ownership/contract text.
- [ ] Avoid branch admission, lab drain, room-time UI, replay playback, and broad `RoomTask`
      refactors.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase file done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server start_payload -- --nocapture`
- Run the exact added or updated focused Rust test names covering:
  - normal live active player start payload;
  - normal live spectator start payload;
  - branch-live active-seat and observer start payloads;
  - lab operator/collaborator start payloads;
  - dev-watch start payload;
  - replay join start payload;
  - replay restart/seek `Start` resend payload.
- If a protocol capability bit is integrated or changed: `node tests/protocol_parity.mjs`.
- `git diff --check`

Do not count a zero-test filter as passed. Do not run broad bundles by default; rely on the PR
`./tests/run-all.sh` gate for full-suite coverage unless the implementation changes a wider
protocol or server-policy contract.

## Manual Test Focus

Start a normal match with one active player and one spectator, then confirm both receive the
expected controls: active player gameplay and live pause controls, spectator read-only controls and
observer diagnostics only when policy allows them.

Launch a replay branch, start the branch-live match, and confirm mapped seats receive active-player
identity, prediction, gameplay, and live pause controls while unmapped joiners remain spectators.

Open a lab room as the first operator, join as a collaborator, and confirm both receive lab metadata,
disabled prediction, and the same lab operation affordances expected after Phase 3. Open a dev-watch
scenario and confirm the watcher receives the dev view player id, dev room-time controls, movement
diagnostics, and no replay or gameplay controls.

Open a replay, confirm the initial join payload has replay metadata, replay room-time controls,
replay vision capability, and diagnostics. Then use replay restart/seek paths and confirm every
resent `Start` payload preserves the same policy-derived capabilities and diagnostics while
updating to the authoritative replay tick.

## Handoff Expectations

Summarize the new builder/helper API, every `ServerMessage::Start` path moved onto it, and any
remaining payload path that intentionally stayed separate. Include the exact focused verification
commands, whether any Phase 4 capability bit required protocol integration, design-doc updates made,
and manual results for live player, live spectator, branch live, lab, dev watch, replay join, and
replay restart/seek.
