# Room Mode Baseline Matrix

This is the Phase 1 current-state baseline for the room/lobby paths that later phases will route
through shared helpers. It records current behavior only; it does not define new room capabilities.

## Summary Matrix

| Path | State Source | Join Behavior | Host / Authority | Commands | Clock | Vision |
| --- | --- | --- | --- | --- | --- | --- |
| Normal lobby | `RoomTask` lobby state with human seats, AI slots, map, quickstart, team, and faction selections. | `RoomMode::Normal` starts in `Phase::Lobby`; joins are accepted unless the room is full, in countdown, draining for new rooms, duplicate, or already in game. Spectators may join before start. | First connected human is host, with host fallback by join order. Host controls AI, teams, map, quickstart, targeted spectator changes, and start. | Gameplay commands are ignored before `Phase::InGame`. Lobby commands are accepted only from the allowed host or active-seat sender. | The room ticker runs at the normal tick interval, but lobby ticks do not advance a simulation. Multiplayer starts use countdown; solo and quickstart starts skip countdown. | Lobby broadcasts are full lobby state, not fog-filtered snapshots. |
| Normal live match | One authoritative `Game` created from active human seats plus AI slots. | Existing lobby connections receive `start`; new joins are rejected while the match is in progress. | Active non-spectator connections issue as their own player ids. Spectators and defeated players are read-only. | Active player commands with nonzero increasing `clientSeq` are enqueued; spectator, stale, zero-sequence, defeated, and non-live commands are ignored. | `LiveTickDriver` advances the game on the room ticker and enqueues AI commands. | Active players receive `snapshot_for(player_id)`. Spectators receive union spectator vision over active seats. |
| Live spectator | Same live `Game` as the normal match. | A lobby spectator remains connected through match start; their `start` payload is stamped `spectator: true`. | No command authority. | Gameplay commands are ignored and no prediction ack is advertised. | Same live ticker as the match. | Receives spectator union snapshots and observer analysis; active players do not receive observer analysis. |
| Post-match replay | `ReplayArtifactV1` captured from the just-finished live `Game`, then rebuilt into `ReplaySession`. | Connected match participants are transitioned into replay playback at tick 0. Later joiners are prompted first unless they confirm replay join. | All viewers are spectators. Any viewer may control playback state under current rules. | Gameplay commands are ignored. Replay speed, seek, vision, and branch request controls are accepted from connected viewers. | `ReplaySession` defaults to replay speed and can pause, seek, and tick until duration end. | Default replay vision is all recorded players. Per-viewer replay vision can select one player or a subset. |
| Persisted replay room | `RoomMode::Replay` with a persisted `ReplayArtifactV1` created by `Lobby::create_replay_room`. | First unconfirmed join receives `joinReplayPrompt`. Confirmed joins start the shared replay viewer runtime; additional confirmed viewers attach to it. | All viewers are spectators. | Same replay controls as post-match replay; gameplay commands are ignored. | Replay ticker, speed, pause, seek, and ended-state behavior come from `ReplaySession`. | Same replay spectator projection and per-viewer vision selection as post-match replay. |
| Saved artifact replay inspection | `RoomMode::ReplayArtifact` loads `target/selfplay-artifacts/<name>/replay.json` or `target/selfplay-failures/<name>/replay.json`. | Launched through `/dev/replay-artifact?replay=<name>`, which maps to a replay-artifact room. Confirmed join starts the shared replay viewer runtime. | All viewers are spectators. | Same replay controls as persisted replay; gameplay commands are ignored. | Same replay ticker as `ReplaySession`. Before the viewer starts, the room still reports the normal tick interval. | Same replay spectator projection and per-viewer vision selection as other replay rooms. |
| Replay branch staging | `RoomMode::ReplayBranch` owns a frozen `ReplayBranchSeed` plus `BranchStagingState`. | First join initializes `Phase::BranchStaging`; all occupants are spectators until launch. Empty branch rooms reset to normal lobby and drop frozen state. | First occupant is host, with host fallback by join order. Occupants claim original replay seats; all claimable seats must be claimed before start. | Normal lobby-only controls are ignored. Claim, release, and host start are accepted under staging rules. | No simulation ticks while staging. Branch start uses the match countdown, then promotes to live. | `branchStaging` messages expose source tick, occupants, seats, claims, host, and can-start state. |
| Replay branch live match | A cloned replay keyframe `Game` from the branch seed. | Claimed occupants become active players; unclaimed occupants remain spectators. | Connection ids map to original replay player ids for commands, snapshots, give-up, and scoring. | Claimed connections issue commands as original replay seats. Branch spectators are read-only. | Same live ticker as normal live play. | Claimed connections receive their mapped original-seat vision. Spectators receive union vision over mapped branch seats. |
| Dev scenario | `RoomMode::DevScenario` builds a scenario `Game` plus `DevScenarioDriver`. | `/dev/scenario` and `/dev/scenarios` rooms auto-start on join. Watchers are spectators. | No normal host/lobby authority. | Normal gameplay commands and lobby controls are ignored. Playback speed and paused step controls are accepted from connected watchers. | Dev watch ticks live unless paused. `stepDevTick` advances exactly one tick while paused. | Watchers receive full-world snapshots for the scenario view player and replay-state style pause/speed messages. |

## Start Payload And Persistence Baseline

| Path | Start Payload Stamping | Mutation Policy | Persistence / History | Empty-Room Reset |
| --- | --- | --- | --- | --- |
| Normal live match | Active players get their own `player_id`, `spectator: false`, prediction build id, and prediction version. Spectators get their connection id with `spectator: true` and prediction disabled. | The live `Game` mutates only inside the room task tick and command paths. | Match history is persisted only when DB is configured, the match started, the mode is not dev watch or replay branch, and the room/participants are not automated test patterns. | Empty live rooms reset to lobby, clear players, AI, match identity, pending acks, and drain tracking. |
| Live spectator | Same live start payload as normal match, stamped as spectator. | Read-only. | Spectators do not change match-history eligibility. | If every connection leaves, the room resets like a live room. |
| Post-match replay | Connected viewers receive replay `start` at tick 0 with replay metadata. | Replay playback rebuilds and ticks a replay `Game`; no live match mutation is accepted. | The replay artifact may be written with the match-history row if persistence is enabled and eligible. | `returnToLobby` removes one viewer; the last viewer resets the room to lobby. |
| Persisted replay room | Confirmed viewers receive replay `start` with replay metadata and spectator stamping. | Read-only replay playback. | Viewing a persisted replay does not create new match-history rows. | Returning/leaving removes viewers; remaining viewers continue playback. |
| Saved artifact replay inspection | Same replay `start` as persisted replay rooms. | Read-only replay playback from the loaded artifact. | No match-history writes. | Same replay viewer detach behavior. |
| Replay branch staging | No `start` payload until promoted to live. | Staging mutates only claims and occupant lists. | No match-history writes while staging. | When empty, resets to `RoomMode::Normal` lobby and discards the seed. |
| Replay branch live match | Claimed occupants get `player_id` stamped as the original replay seat, `spectator: false`, prediction enabled, and no replay metadata. Unclaimed occupants get spectator start payloads. | Live branch `Game` mutates like normal live play, using original replay seat ids. | Public match-history persistence is skipped for replay branch live matches. | Empty live branch rooms reset live bookkeeping; branch live mappings are cleared after match end or reset. |
| Dev scenario | Watchers get the scenario view player id, `spectator: true`, prediction disabled, and no replay metadata. | Scenario driver and tick path mutate the dev `Game`; user gameplay commands are ignored. | Dev watch is excluded from match-history persistence and drain active-match tracking. | Empty dev scenario rooms reset in-game state and dev driver/viewer bookkeeping. |

## Focused Baseline Tests

These tests are the current executable baseline for the high-risk behavior later phases will route
through shared helpers:

- Join routing and replay prompts: `persisted_replay_room_join_prompts_before_playback`,
  `persisted_replay_room_confirmed_join_starts_replay_viewer`,
  `saved_artifact_replay_join_uses_replay_viewer_runtime`,
  `replay_branch_room_join_initializes_staging_and_broadcasts_seats`, and
  `post_match_replay_join_prompts_before_attaching_viewer`.
- Command authority and read-only paths: `lobby_phase_ignores_gameplay_commands`,
  `normal_live_spectator_start_payload_is_read_only`, `replay_phase_ignores_gameplay_commands`,
  and `branch_live_commands_and_snapshots_use_mapped_original_seats`.
- Branch seat aliasing and staging: `branch_launch_preparation_preserves_original_replay_seat_mapping`,
  `branch_live_commands_and_snapshots_use_mapped_original_seats`,
  `branch_live_give_up_resolves_by_original_seat_and_skips_public_history`,
  `branch_staging_seat_claims_are_exclusive`, and
  `branch_staging_requires_all_original_seats_before_can_start`.
- Replay vision and fanout: `replay_vision_selection_is_per_viewer`,
  `rapid_replay_vision_changes_remain_per_viewer`,
  `replay_viewer_snapshot_hides_resource_outside_union_fog`, and
  `single_player_replay_fog_matches_player_visibility`.
- Dev pause and step: `paused_dev_scenario_steps_one_tick_at_a_time`.
- Spectator projection and analysis: `live_spectator_receives_observer_analysis_but_active_players_do_not`
  and `normal_live_spectator_start_payload_is_read_only`.
- Match-history decisions: `match_history_persistence_allows_solo_and_human_ai_matches`,
  `match_history_persistence_allows_ai_only_but_skips_test_matches`, and
  `branch_live_give_up_resolves_by_original_seat_and_skips_public_history`.

## Manual Smoke Still Required

- Normal lobby start with two humans and with one human plus AI.
- Spectator join before start, then live spectator view after the match starts.
- Post-match replay prompt and return-to-lobby behavior.
- Persisted replay launch from match history.
- Replay branch staging, seat claim/release, launch, and live command control.
- Saved artifact replay inspection through `/dev/replay-artifact?replay=<name>`.
- One `/dev/scenario` URL, including pause, step, resume, and speed controls.

## Contract Notes

- Wire protocol shape is unchanged.
- `Game` API shape is unchanged.
- Gameplay rules are unchanged.
- Visible client behavior is intended to remain unchanged; this document and the added tests only
  characterize existing behavior.
