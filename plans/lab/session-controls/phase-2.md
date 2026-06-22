# Phase 2 - Quickstart Protocol And Loadout Removal

## Phase Status

- [ ] Pending.

## Objective

Delete the legacy quickstart/debug mode from active protocol, server, client, tests, and
source-of-truth docs.

## Work

- Remove `setQuickstart` from Rust protocol DTOs, compact/protocol parity fixtures, server message
  handling, JavaScript protocol builders, and `Net`.
- Remove `quickstart` from active lobby state, `lobby` protocol payloads, client lobby state, and any
  optional hidden quickstart DOM compatibility.
- Remove `RoomTask::quickstart`, `on_set_quickstart`, quickstart countdown skipping, quickstart
  faction validation context, and quickstart-specific movement diagnostics.
- Remove the debug starting loadout path and boosted-resource constants if they no longer have a
  non-quickstart owner. If a lower-level helper is still needed by an isolated sim unit test, rename it
  as a test fixture and keep it unavailable to live rooms.
- Keep historical database/schema fields only as historical compatibility if deleting them would
  require a migration. New live matches should not write `debug_mode = true` because the product path
  no longer exists.
- Update `docs/design/protocol.md`, `docs/design/client-ui.md`, `docs/design/server-sim.md`,
  `docs/design/match-history.md`, relevant context capsules, and `client/index.html` comments so
  quickstart is not described as an active compatibility command.
- Do not create lab presets in this phase. The replacement for old debug-style setups is later
  hand-authored lab scenarios, not a hidden quickstart clone.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/crates/contract/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/main.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/faction_validation.rs`
- `server/src/config.rs`
- `server/crates/sim/src/game/setup/` if the debug loadout helper is deleted or renamed
- `client/src/protocol.js`
- `client/src/net.js`
- `client/src/lobby.js`
- `client/index.html`
- `tests/protocol_parity.mjs`
- `tests/client_contracts/*.mjs`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`
- `docs/design/match-history.md`
- `docs/context/*.md` where section references shift

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim debug` if sim debug-loadout helpers are
  touched and the filter matches tests
- `node tests/select-suites.mjs --verify`
- `rg -n "setQuickstart|quickstart|QUICKSTART|Debug mode|debug_mode" server client tests docs/design docs/context`
- `git diff --check`

The final search may still find historical DB column names or archived files. The handoff must
separate acceptable historical leftovers from active product/protocol references.

## Manual Test Focus

Open the normal lobby and confirm no quickstart/debug control or compatibility copy is present. Start
a solo normal room and confirm it uses normal setup while still reaching the match screen.

## Handoff Expectations

State exactly which protocol fields/tags were removed, whether any historical `debug_mode` storage
remains, and where future hand-authored lab presets should hook in without reviving quickstart.
