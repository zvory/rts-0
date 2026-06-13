# Faction Lifecycle Matrix

Status: Initial inventory complete in Phase 0. Every later phase that touches a match lifecycle
path must keep this matrix updated.

## Purpose

Track the source of faction truth for every path that can create, replay, inspect, or branch a
match. Each row must say whether a path is supported, defaulted to the current faction, dev-gated,
or explicitly rejected.

## Columns

- `Path`: user-visible or internal flow.
- `Faction source`: where the authoritative faction id comes from.
- `Allowed factions`: current only, fixture/dev only, selected playable factions, or recorded
  replay factions.
- `AI behavior`: accepted, rejected, current-faction-only, or not applicable.
- `Prediction behavior`: enabled, disabled with reason, or not applicable.
- `Replay/branch behavior`: recorded, reconstructed from schema, rejected, or not applicable.
- `Tests`: focused test or checker that owns the row.
- `Notes`: temporary shims, rollout gates, or follow-up work.

## Initial Rows

| Path | Faction source | Allowed factions | AI behavior | Prediction behavior | Replay/branch behavior | Tests | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Normal lobby start | `LobbyPlayer.factionId` and `PlayerInit.faction_id`, defaulted to `steel_vanguard` | Current faction | Current-faction-only | Enabled only for supported faction/build | Schema 2 records player faction id | `tests/server_integration.mjs` | Selection UX remains hidden until rollout. |
| Quickstart/debug start | `PlayerInit.faction_id`, defaulted to `steel_vanguard`; debug loadout remains separate | Current faction; fixture only if explicitly exposed later | Current-faction-only | Disabled for unsupported fixtures | Schema 2 records faction id and current loadout shim | `server/crates/sim/src/game/setup/tests.rs` | No implicit second-faction quickstart. |
| AI add/remove/start | AI `PlayerInit.faction_id`, defaulted to `steel_vanguard` at match start | Current faction only | Current-faction-only | Not applicable | Schema 2 records AI faction if match starts | `tests/server_integration.mjs`, future Phase 3 validation test | Must fail closed once non-default ids are exposed. |
| Fixture/dev faction start | No explicit faction source yet | Not supported until a dev/test fixture phase | Reject AI unless explicitly allowed | Disabled unless WASM supports fixture | Record fixture id/loadout if replayed later | `scripts/check-faction-assumptions.mjs`, future fixture tests | Used before real lobby selection exists. |
| Replay playback | `ReplayArtifactV1.players[].faction_id` in artifact schema 2 | Recorded replay factions | No new AI assignment | Disabled for viewer prediction | Load from artifact schema, never lobby state; schema 1 rejected | `server/crates/sim/src/game/replay.rs` tests | Old artifacts without faction ids are incompatible. |
| Replay branch staging/launch | Branch seed seats copy recorded `factionId` from replay players | Recorded replay factions | Reject AI seats unless explicitly supported later | Disabled unless supported by branch schema/WASM | Reconstruct from branch seed and cloned keyframe | `server/src/lobby/room_task.rs` tests, `tests/protocol_parity.mjs` | Seat claims do not alter faction ids. |
| Dev scenarios | Scenario definition plus current default start | Current faction | Not applicable unless scenario declares AI | Disabled unless supported | Not replayed unless scenario recording exists | `server/crates/sim/src/game/setup/dev_scenarios/tests.rs`, `docs/context/testing.md` | No arbitrary client spawning. |
| Self-play | Self-play `PlayerInit.faction_id`, defaulted to `steel_vanguard` | Current faction until AI support expands | Current-faction-only | Not applicable | Artifact schema 2 records faction ids | `server/crates/ai/src/selfplay` tests, future Phase 3/12 artifact tests | Separate AI plan needed for other factions. |
| Match history replay | Stored schema-2 match artifact | Recorded replay factions | From artifact only | Disabled for replay viewers | Load from persisted schema; schema 1 rejected | `docs/design/match-history.md`, future Phase 3/12 match-history tests | Old persisted replays are incompatible. |
| Spectator/no-fog view | Live match start payload or replay schema | Match factions from start/replay metadata | Not applicable | Disabled | Preserve recorded faction metadata | `tests/server_integration.mjs`, future observer faction metadata test | Resource rows stay Steel/Oil/Supply. |
| Post-match replay | Captured schema-2 match artifact | Recorded replay factions | From artifact only | Disabled for replay viewers | Load from captured schema | `tests/server_integration.mjs` | Same schema as match history replay. |
