# Phase 1 - Lobby Team Assignment

Goal: make teams configurable in the lobby while keeping the backend assignment generic.

The host should be able to choose FFA, 1v2, 1v3, or 2v2 and add an AI to any team. Non-hosts can
see teams but cannot mutate them.

## Server Lobby Behavior

Update `server/src/lobby/`.

Room state should store:

- Current team preset, default `ffa`
- Human `RoomPlayer.team_id`
- `AiSlot.team_id`

Presets are lobby conveniences:

- `ffa`: every seated player has a unique team.
- `1v2`: Team 1 has one seat, Team 2 has two seats.
- `1v3`: Team 1 has one seat, Team 2 has three seats.
- `2v2`: Team 1 has two seats, Team 2 has two seats.

When the host changes preset:

- Reassign current seated players in lobby order.
- Keep the host on Team 1 when possible.
- Assign later humans and AIs into the remaining preset seats in order.
- If the room has too many seated players for the chosen preset, reject the preset change and send
  a notice/error to the host.

When the host calls `addAi`:

- If `teamId` is supplied and valid for the current preset or generic assignment, seat the AI there.
- If `teamId` is omitted, use the current default behavior for FFA and the first open preset seat for
  team modes.
- Keep AI rows ready and removable as today.

When the host calls `setTeam`:

- Require lobby phase.
- Require host.
- Require target id to be a seated human or AI.
- Require nonzero team id.
- For preset modes, require the resulting team sizes to stay within the preset limits.
- For FFA, moving a player onto another occupied team effectively creates a non-FFA generic team
  assignment. The short-run UI does not need to expose this, but the backend should tolerate it.

## Start Eligibility

All existing ready rules still apply: all connected humans must be ready.

Preset-specific rules:

- Solo sandbox: one seated human, no AI required, starts as today.
- FFA: 1 through current map capacity seated players may start.
- 1v2: exactly three seated players, Team 1 size one, Team 2 size two.
- 1v3: exactly four seated players, Team 1 size one, Team 2 size three.
- 2v2: exactly four seated players, Team 1 size two, Team 2 size two.

Generic backend rule for future custom layouts:

- One seated player is allowed as sandbox.
- Otherwise, at least two nonempty teams must be present.

## Client Lobby UI

Update `client/index.html`, `client/styles.css`, and `client/src/lobby.js`.

Expected short-run UI:

- A host-only preset segmented control: `FFA`, `1v2`, `1v3`, `2v2`.
- Team columns or grouped rows in the player list.
- Host-only "Add AI" control per team.
- Host-only row movement control if a row is in the wrong team for the selected preset.
- Non-hosts see the same grouping but no mutation controls.

Keep the lobby utilitarian. This is an operational pre-match screen, not a landing page.

## Files to Touch

- `docs/design/*.md`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/lobby/`
- `client/src/protocol.js`
- `client/src/net.js`
- `client/src/lobby.js`
- `client/index.html`
- `client/styles.css`
- `tests/ai_integration.mjs`
- `tests/server_integration.mjs`

## Tests

Add or update integration coverage:

- Default room is FFA and assigns unique team ids.
- Non-host `setTeamPreset`, `setTeam`, and `addAi(teamId)` are ignored.
- Host can select 1v2, add two AIs to Team 2, ready, and start.
- Host can select 1v3, add three AIs to Team 2, ready, and start.
- Host can select 2v2 and add AIs to fill open seats.
- Invalid team id `0` is rejected.
- Invalid preset start leaves `canStart` false.

Run:

```bash
cd server && cargo test
node tests/server_integration.mjs
node tests/ai_integration.mjs
```

## Acceptance Criteria

- The lobby can produce valid `PlayerInit { team_id }` lists for FFA, 1v2, 1v3, and 2v2.
- Host can add AI to a specific team.
- Non-hosts cannot alter teams.
- The current solo sandbox still starts.
- No in-game team behavior is expected yet beyond team ids appearing in the start payload.
