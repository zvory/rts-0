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
| Normal lobby start | Current implicit default in lobby room start | Current faction | Current-faction-only | Enabled only for supported faction/build | Future schema records faction id | `tests/server_integration.mjs`, future Phase 1 start contract test | Selection UX may remain hidden until rollout. |
| Quickstart/debug start | Current implicit default plus debug loadout flag | Current faction; fixture only if explicitly exposed later | Current-faction-only | Disabled for unsupported fixtures | Future schema records faction id and loadout | `server/crates/sim/src/game/setup/tests.rs`, future Phase 1/5 tests | No implicit second-faction quickstart. |
| AI add/remove/start | Current lobby AI seat creation with implicit faction | Current faction only | Reject unsupported faction once ids exist | Not applicable | Future schema records AI faction if match starts | `node tests/ai_integration.mjs`, future Phase 3 validation test | Must fail closed. |
| Fixture/dev faction start | No explicit faction source yet | Not supported until a dev/test fixture phase | Reject AI unless explicitly allowed | Disabled unless WASM supports fixture | Record fixture id/loadout if replayed later | `scripts/check-faction-assumptions.mjs`, future fixture tests | Used before real lobby selection exists. |
| Replay playback | Current replay artifact start resources and loadout mode | Current faction until schema is replaced | No new AI assignment | Disabled for viewer prediction | Load from artifact schema, never lobby state | `server/src/lobby/dev_replay.rs` tests, future Phase 1 replay schema test | Old artifacts may be incompatible. |
| Replay branch staging/launch | Current branch seed/keyframe plus claimed seats | Current faction until schema is replaced | Reject AI seats unless explicitly supported later | Disabled unless supported by branch schema/WASM | Reconstruct from branch seed | `tests/protocol_parity.mjs`, future Phase 3 branch tests | Seat claims do not alter faction ids. |
| Dev scenarios | Scenario definition plus current default start | Current faction | Not applicable unless scenario declares AI | Disabled unless supported | Not replayed unless scenario recording exists | `server/crates/sim/src/game/setup/dev_scenarios/tests.rs`, `docs/context/testing.md` | No arbitrary client spawning. |
| Self-play | Self-play script/profile using current faction assumptions | Current faction until AI support expands | Current-faction-only | Not applicable | Artifact records faction ids after schema changes | `server/crates/ai/src/selfplay` tests, future Phase 3/12 artifact tests | Separate AI plan needed for other factions. |
| Match history replay | Stored match artifact | Current faction until schema is replaced | From artifact only | Disabled for replay viewers | Load from persisted schema | `docs/design/match-history.md`, future Phase 3/12 match-history tests | Old persisted replays may be incompatible. |
| Spectator/no-fog view | Live match state or replay schema | Match factions once schema exists | Not applicable | Disabled | Preserve recorded faction metadata | `tests/team_integration.mjs`, future observer faction metadata test | Resource rows stay Steel/Oil/Supply. |
| Post-match replay | Captured match artifact | Current faction until schema is replaced | From artifact only | Disabled for replay viewers | Load from captured schema | `tests/server_integration.mjs`, future Phase 3 post-match replay test | Same schema as match history replay. |
