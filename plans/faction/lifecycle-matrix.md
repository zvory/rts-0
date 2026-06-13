# Faction Lifecycle Matrix

Status: Template, to be filled during Phase 0 and maintained by every later phase that touches a
match lifecycle path.

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
| Normal lobby start | Default assignment until selection UX exists | Current faction | Current-faction-only | Enabled only for supported faction/build | Record faction id in schema | TBD | Selection UX may remain hidden until rollout. |
| Quickstart/debug start | Default assignment plus debug loadout policy | Current faction, fixture only if explicitly exposed | Current-faction-only | Disabled for unsupported fixtures | Record faction id and loadout | TBD | No implicit second-faction quickstart. |
| AI add/remove/start | Server validation helper | Current faction only | Reject unsupported faction | Not applicable | Record AI faction if match starts | TBD | Must fail closed. |
| Fixture/dev faction start | Explicit dev/test request | Fixture/dev factions only | Reject AI unless explicitly allowed | Disabled unless WASM supports fixture | Record fixture id/loadout if replayed | TBD | Used before real lobby selection exists. |
| Replay playback | Recorded replay artifact schema | Recorded factions | No new AI assignment | Disabled for viewer prediction | Load from schema, never lobby state | TBD | Old artifacts may be incompatible. |
| Replay branch staging/launch | Recorded branch seed schema and claimed seats | Recorded factions | Reject AI seats unless explicitly supported | Disabled unless supported by branch schema/WASM | Reconstruct from branch seed | TBD | Seat claims do not alter faction ids. |
| Dev scenarios | Scenario definition | Scenario-declared faction or current default | Not applicable unless scenario declares AI | Disabled unless supported | Not replayed unless scenario recording exists | TBD | No arbitrary client spawning. |
| Self-play | Self-play script/profile | Current faction until AI support expands | Current-faction-only | Not applicable | Artifact records faction ids | TBD | Separate AI plan needed for other factions. |
| Match history replay | Stored match artifact | Recorded factions | From artifact only | Disabled for replay viewers | Load from persisted schema | TBD | Old persisted replays may be incompatible. |
| Spectator/no-fog view | Live match state or replay schema | Match factions | Not applicable | Disabled | Preserve recorded faction metadata | TBD | Resource rows stay Steel/Oil/Supply. |
| Post-match replay | Captured match artifact | Recorded factions | From artifact only | Disabled for replay viewers | Load from captured schema | TBD | Same schema as match history replay. |
