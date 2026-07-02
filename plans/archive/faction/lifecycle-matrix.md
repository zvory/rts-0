# Faction Lifecycle Matrix

Status: Updated through Phase 4 resource-policy hardening. Every later phase that touches a match
lifecycle path must keep this matrix updated.

## Purpose

Track the source of faction truth for every path that can create, replay, inspect, or branch a
match. Each row must say whether a path is supported, defaulted to the current faction, dev-gated,
or explicitly rejected.

Canonical faction ids:

- Existing faction: `kriegsia` (**Kriegsia**).
- Reserved future faction: `ekat` (**Ekat**), not playable until later approved phases.
- Architecture fixture: `phase2_empty_fixture`, test/dev-only and not a product faction.

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
| Normal lobby start | `LobbyPlayer.factionId` and `PlayerInit.faction_id`, defaulted by `lobby::faction_validation`; start state comes from the Kriegsia catalog loadout | Kriegsia only; `ekat` and fixture ids reject | Kriegsia-only | Enabled for local Kriegsia player when build/version metadata is compatible | Then-current replay schema records player faction id plus per-player loadout record | `tests/server_integration.mjs`, `tests/prediction_controller.mjs`, `server/src/lobby/faction_validation.rs` tests | Selection UX remains hidden until rollout. |
| Quickstart/debug start | `PlayerInit.faction_id`, defaulted by `lobby::faction_validation`; debug extras layer on the Kriegsia catalog loadout | Kriegsia only; fixture and `ekat` reject | Kriegsia-only | Disabled only for unsupported local-player faction | Then-current replay schema records faction id and per-player debug/standard loadout records | `server/crates/sim/src/game/setup/tests.rs`, `server/src/lobby/faction_validation.rs` tests | No implicit Ekat or fixture quickstart. |
| AI add/remove/start | AI `PlayerInit.faction_id`, defaulted by `lobby::faction_validation` at match start | Kriegsia only; unsupported ids reject before future seat creation paths can use them | Kriegsia-only | Not applicable | Then-current replay schema records AI faction and per-player loadout if match starts | `tests/ai_integration.mjs`, `tests/server_integration.mjs`, `server/src/lobby/faction_validation.rs` tests | `addAi` remains team-only in Phase 3. |
| Fixture/dev faction start | Explicit Rust test/dev harness only; fixture catalog loadout starts with alternate Steel/Oil, Depot, and Scout Car | `phase2_empty_fixture` accepted only by `TestFixture` validation context or direct sim tests; not normal lobby selectable | Reject AI unless explicitly allowed by a later phase | Disabled when local fixture player is unsupported | Record fixture id/loadout only in explicit test artifacts | `server/crates/sim/src/game/setup/tests.rs`, `tests/prediction_controller.mjs`, `scripts/check-faction-assumptions.mjs`, `server/src/lobby/faction_validation.rs` tests | Fixture is architecture coverage, not a product faction. |
| Replay playback | `ReplayArtifactV1.players[].faction_id` and `playerLoadouts[]` in the then-current artifact schema, validated before playback | Recorded Kriegsia in live replay paths; unknown, fixture-only, and unsupported catalog ids reject | No new AI assignment | Disabled for viewer prediction | Load from artifact schema, never lobby state; older or missing/mismatched faction/loadout records rejected | `server/crates/sim/src/game/replay.rs` tests, `server/src/lobby/room_task.rs` replay tests | Old artifacts without faction ids or loadout records are incompatible. |
| Replay branch staging/launch | Branch seed seats copy recorded `factionId` from replay players and validate before launch | Recorded Kriegsia in Phase 3D; unsupported seat faction ids reject before live launch | Reject AI seats unless explicitly supported later | Disabled unless supported by branch schema/WASM | Reconstruct from branch seed and cloned keyframe | `server/src/lobby/room_task.rs` tests, `tests/protocol_parity.mjs` | Seat claims do not alter faction ids. |
| Dev scenarios | Scenario definition plus Kriegsia default start unless scenario explicitly owns a test fixture | Kriegsia only in Phase 3D; fixture rejected outside explicit test fixture context | Not applicable unless scenario declares AI | Disabled only for unsupported local-player faction | Not replayed unless scenario recording exists | `server/crates/sim/src/game/setup/dev_scenarios/tests.rs`, `docs/context/testing.md` | No arbitrary client spawning. |
| Self-play | Self-play `PlayerInit.faction_id`, validated by `lobby::faction_validation` | Kriegsia until AI support expands | Kriegsia-only | Not applicable | Then-current artifact schema records faction ids and per-player loadouts | `server/crates/ai/src/selfplay` tests | Separate AI plan needed for other factions. |
| Match history replay | Stored replay match artifact | Recorded Kriegsia in live replay paths; unknown, fixture-only, and unsupported catalog ids reject at replay launch | From artifact only | Disabled for replay viewers | Load from persisted schema; older schemas rejected by decode/compatibility and unsupported or missing/mismatched faction/loadout rows reject before room creation | `server/src/main.rs` replay compatibility tests, `docs/design/match-history.md` | Old persisted replays without loadout records are incompatible. |
| Spectator/no-fog view | Live match start payload or replay schema | Match factions from start/replay metadata | Not applicable | Disabled | Preserve recorded faction metadata | `tests/server_integration.mjs`, future observer faction metadata test | Resource rows stay fixed Steel/Oil/Supply fields. |
| Post-match replay | Captured match artifact | Recorded replay factions | From artifact only | Disabled for replay viewers | Load from captured schema with Steel/Oil/Supply resource payloads | `tests/server_integration.mjs` | Same schema as match history replay; generic resources are deferred. |
