# Phase 2 - Join Notice And Lifecycle Polish

## Phase Status

- [x] Done.

## Objective

Notify existing match recipients when someone joins an already-running normal match as a spectator.
The notification must read `<name> has joined the match as a spectator`, use `Commander` when the
join name is blank or otherwise unavailable, and avoid adding any modal or extra prompt.

## Work

- Add a server-owned notice path for late spectator joins:
  - capture the target recipient ids before inserting the new spectator;
  - target all active player connections and already-connected spectators;
  - exclude the newly joined spectator unless a deliberate product decision changes the requirement;
  - use `NoticeSeverity::Info`;
  - leave `x` and `y` unset;
  - do not use an `alert:` prefix.
- Reuse existing snapshot `Event::Notice` handling if practical:
  - add a small room-owned pending recipient notice queue keyed by connection id;
  - append queued notices to that recipient's next live snapshot after normal projection;
  - clear notices after successful fanout so they do not repeat every tick;
  - make sure spectator union event logic does not accidentally include or drop targeted notices.
- Handle live pause deliberately:
  - do not advance the simulation just to flush a notice;
  - if snapshots are paused, keep the notice queued until the next emitted live snapshot after
    unpause;
  - document this behavior unless the implementation adds a narrower reliable notice path.
- Normalize the displayed spectator name:
  - use the sanitized WebSocket join name when available;
  - trim and reject empty/control-only results;
  - fall back to `Commander`;
  - do not add a modal, prompt, or client-only name confirmation.
- Add tests for notice targeting:
  - active players receive the notice;
  - already-connected spectators receive the notice;
  - the newly joined spectator does not receive the notice unless the phase intentionally changes
    the requirement;
  - blank names produce `Commander has joined the match as a spectator`;
  - notices are one-shot and do not repeat across multiple snapshots;
  - active late joins still do not emit spectator notices.
- Close lifecycle polish:
  - verify disconnecting a late spectator does not eliminate any army;
  - verify room empty reset still returns the room to a clean lobby;
  - verify match-history participants and match-player counts remain based on active seats only;
  - verify lobby browser spectator counts update on the next HTTP poll when late spectators join or
    leave during a live match.
- Update docs:
  - `docs/design/protocol.md` for the notice text and delivery semantics if using `Event::Notice`;
  - `docs/design/server-sim.md` for room-owned pending recipient notices;
  - `docs/design/client-ui.md` only if client notice handling or browser copy changes.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/snapshot_fanout.rs` if recipient-specific post-projection event injection is
  cleaner there
- `server/crates/contract/src/lib.rs` only if a new server-message notice path becomes necessary
- `server/crates/protocol/src/lib.rs` and `client/src/protocol.js` only if a new reliable notice
  message becomes necessary
- `client/src/match.js` only if the existing `Event::Notice` path is insufficient
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `tests/client_contracts.mjs` if client notice behavior changes
- `tests/lobby_browser_integration.mjs`
- `tests/server_integration.mjs` or focused Rust tests for notice fanout

## Implementation Checklist

- [x] Mid-match spectator joins enqueue the exact info notice text.
- [x] Blank or unavailable names fall back to `Commander`.
- [x] Active players receive the notice.
- [x] Existing spectators receive the notice.
- [x] The newly joined spectator is excluded unless the implementation records an intentional
      product change.
- [x] Notice delivery is one-shot.
- [x] Paused-match notice behavior is covered and documented.
- [x] Late spectator disconnects do not affect active armies or match outcomes.
- [x] Docs and tests are updated.
- [x] Verification is run and recorded.
- [x] This phase file is marked done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server late_spectator_notice -- --nocapture`
- `cargo test --manifest-path server/Cargo.toml -p rts-server live_pause -- --nocapture` if queued
  notices interact with paused live matches
- `node tests/lobby_browser_integration.mjs` with a running server
- `node tests/server_integration.mjs` with a running server if live spectator snapshot assertions are
  extended there
- `node tests/client_contracts.mjs` if any client notice behavior changes
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If a focused Rust filter matches zero tests, run the exact test names added in this phase before
counting verification as passed.

## Manual Test Focus

Run a local match with one active player plus AI or two active players, then join from another
browser as a spectator after the match starts. Confirm every already-connected active player and
spectator sees `<name> has joined the match as a spectator`, the joining spectator does not get a
confusing self-notice, and blank-name joins display `Commander`. Repeat once while the match is
paused if live pause is available; confirm the notice behavior matches the documented paused-match
decision.

## Handoff Expectations

Report the exact notice delivery mechanism, whether paused matches queue or immediately deliver the
notice, and the tests proving recipient targeting. Include verification commands, manual-test
results if performed, and any remaining risk around high spectator counts or snapshot fanout cost.
