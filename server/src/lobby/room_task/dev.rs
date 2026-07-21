use std::{collections::HashMap, time::Instant as StdInstant};

use tokio::time::Instant as TokioInstant;

use super::super::connection::{send_or_log, ConnectionSink};
use super::super::crash_replay::{dump_crash_replay, panic_reason};
use super::super::dev_replay::match_seed;
use super::super::dev_scenario_id::DevScenarioId;
use super::super::faction_validation::{default_faction_id_for, FactionRequestContext};
use super::super::launch::{LaunchRecipient, StartPayloadBuilder};
use super::super::projection::{observer_view_or_all, RecipientRole};
use super::super::snapshot_fanout::{SnapshotFanout, SnapshotFanoutPayload};
use super::super::snapshots::union_events;
use super::types::{Phase, RoomMode, RoomPlayer};
use super::RoomTask;
use crate::protocol::{Event, ServerMessage};
use rts_sim::game::{command::SimCommand, Game};

pub(super) enum DevDriver {
    Scenario(DevScenarioDriver),
}

impl DevDriver {
    fn enqueue_for_tick(&mut self, game: &mut Game) {
        match self {
            DevDriver::Scenario(scenario) => scenario.enqueue_for_tick(game),
        }
    }
}

pub(super) struct DevScenarioDriver {
    player_id: u32,
    command: SimCommand,
    issue_after_ticks: u32,
    issued: bool,
}

impl DevScenarioDriver {
    fn enqueue_for_tick(&mut self, game: &mut Game) {
        if self.issued {
            return;
        }
        if game.tick_count() < self.issue_after_ticks {
            return;
        }
        self.issued = true;
        game.enqueue(self.player_id, self.command.clone());
    }
}

impl RoomTask {
    pub(super) fn on_join_dev_watch(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.players.contains_key(&player_id) {
            let _ = ack.send(false);
            return;
        }
        self.order.push(player_id);
        self.players.insert(
            player_id,
            RoomPlayer {
                name,
                color: "#6f8fa8".to_string(),
                ready: true,
                spectator: true,
                msg_tx,
                head_of_line_count: 0,
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        let _ = ack.send(true);
        if !matches!(self.phase, Phase::InGame(_)) {
            self.start_dev_session();
        } else {
            self.send_dev_start_to(player_id);
        }
    }

    fn start_dev_session(&mut self) {
        self.prepare_live_match_launch();
        let (game, driver, view_player_id) = match self.build_dev_session() {
            Ok(session) => session,
            Err(err) => {
                crate::log_warn!(room = %self.room, error = %err, "dev session bootstrap failed");
                self.send_dev_error(&err);
                return;
            }
        };
        self.mark_match_started_for_drain();
        self.capture_replay_start_for(&game);
        self.phase = Phase::InGame(Box::new(game));
        self.match_player_count = 2;
        self.dev_driver = Some(driver);
        self.dev_view_player_id = Some(view_player_id);
        self.ai_controllers.clear();
        let recipients = self.order.clone();
        for player_id in recipients {
            self.send_dev_start_to(player_id);
        }
        crate::log_info!(room = %self.room, "dev session started");
    }

    fn build_dev_session(&self) -> Result<(Game, DevDriver, u32), String> {
        match &self.mode {
            RoomMode::Normal
            | RoomMode::Replay { .. }
            | RoomMode::ReplayArtifact { .. }
            | RoomMode::ReplayBranch { .. }
            | RoomMode::Lab(_) => Err("room is not configured for a dev session".to_string()),
            RoomMode::DevScenario(config) => {
                let _ = default_faction_id_for(FactionRequestContext::DevScenario);
                let seed = match_seed();
                macro_rules! session_from_setup {
                    ($setup:expr $(,)?) => {{
                        let setup = $setup;
                        let player_id = setup.player_id;
                        let command = setup.command();
                        let driver = DevScenarioDriver {
                            player_id,
                            command,
                            issue_after_ticks: setup.issue_after_ticks,
                            issued: false,
                        };
                        Ok((setup.game, DevDriver::Scenario(driver), player_id))
                    }};
                }

                match &config.id {
                    DevScenarioId::DynamicConstructionPathBlock => {
                        session_from_setup!(Game::new_dynamic_construction_path_block_scenario(
                            config.case,
                            config.unit,
                            config.count,
                            seed,
                        )?,)
                    }
                    DevScenarioId::ScoutCarSnakingCorridor => session_from_setup!(
                        Game::new_snaking_corridor_scenario(config.unit, config.count, seed)?,
                    ),
                    DevScenarioId::DirectReverseOrder => session_from_setup!(
                        Game::new_direct_reverse_order_scenario(config.unit, config.count, seed)?,
                    ),
                    DevScenarioId::Replay142VehicleLock => {
                        session_from_setup!(Game::new_replay_142_vehicle_lock_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?,)
                    }
                    DevScenarioId::ScoutCarWallChokepoint => {
                        session_from_setup!(Game::new_scout_car_wall_chokepoint_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::VehicleCornerWall => session_from_setup!(
                        Game::new_vehicle_corner_wall_scenario(config.unit, config.count, seed)?,
                    ),
                    DevScenarioId::VehicleSmallBlockBaseline => {
                        session_from_setup!(Game::new_vehicle_small_block_baseline_scenario(
                            config.unit,
                            config.count,
                            config.blocker,
                            seed,
                        )?)
                    }
                    DevScenarioId::FactoryZeroGapPerpendicular => {
                        session_from_setup!(Game::new_factory_zero_gap_perpendicular_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::CommandCarBuildingCorner => session_from_setup!(
                        Game::new_command_car_corner_scenario(config.unit, config.count, seed)?,
                    ),
                    DevScenarioId::CommandCarBuildingCornerWestSouthwest => {
                        session_from_setup!(Game::new_command_car_corner_west_southwest_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?,)
                    }
                    DevScenarioId::FactoryWallRallySpawn => {
                        session_from_setup!(Game::new_factory_wall_rally_spawn_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::TankTrapLineHorizontal
                    | DevScenarioId::TankTrapLineVertical
                    | DevScenarioId::TankTrapLineDiagonal => {
                        session_from_setup!(Game::new_tank_trap_line_build_scenario(
                            config.id.room_id(),
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::TankTrapPathingMatrix => {
                        let scenario_case = config
                            .case
                            .ok_or_else(|| "missing Tank Trap pathing case".to_string())?;
                        session_from_setup!(Game::new_tank_trap_pathing_scenario(
                            scenario_case,
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::EntrenchmentInspection => {
                        session_from_setup!(Game::new_entrenchment_inspection_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::TankCoaxInspection => {
                        session_from_setup!(Game::new_tank_coax_inspection_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::AttackMoveReloadAcquisition => {
                        session_from_setup!(Game::new_attack_move_reload_acquisition_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::TankUnderFireRetreat => {
                        session_from_setup!(Game::new_tank_under_fire_retreat_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                    DevScenarioId::TankReverseTraffic => {
                        session_from_setup!(Game::new_tank_reverse_traffic_scenario(
                            config.unit,
                            config.count,
                            seed,
                        )?)
                    }
                }
            }
        }
    }
    fn send_dev_start_to(&self, watcher_id: u32) {
        let Some(Phase::InGame(game)) = Some(&self.phase) else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        let payload = game.start_payload();
        let role = RecipientRole::Spectator;
        let diagnostics = self.projection_policy().diagnostic_capabilities_for(role);
        let start_policy = self.session_policy();
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::super::launch::send_start_payloads(
            &self.room,
            &builder,
            [LaunchRecipient::observer(
                watcher_id,
                diagnostics,
                false,
                None,
                self.observer_view_selection_for(watcher_id),
                player.msg_tx.clone(),
            )],
        );
    }

    pub(super) fn broadcast_dev_watch_state(&self) {
        if !self.session_policy().is_dev_watch() {
            return;
        }
        let Phase::InGame(game) = &self.phase else {
            return;
        };
        self.broadcast(&ServerMessage::RoomTimeState(
            self.room_time_state_for_live_game(game, None),
        ));
    }

    pub(super) fn send_dev_error(&self, msg: &str) {
        let payload = ServerMessage::Error {
            msg: msg.to_string(),
        };
        for &watcher_id in &self.order {
            let Some(player) = self.players.get(&watcher_id) else {
                continue;
            };
            send_or_log(&self.room, watcher_id, &player.msg_tx, payload.clone());
        }
    }

    pub(super) fn on_tick_dev_watch(&mut self, scheduled: TokioInstant) {
        let mut game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::Lobby => return,
            Phase::InGame(game) => game,
            Phase::ReplayViewer(session) => {
                self.phase = Phase::ReplayViewer(session);
                return;
            }
            Phase::BranchStaging(staging) => {
                self.phase = Phase::BranchStaging(staging);
                return;
            }
        };
        let scheduler_lag = scheduled.elapsed();
        let tick_start = StdInstant::now();
        let mut perf = rts_sim::perf::TickPerf::maybe_new();
        let Some(mut driver) = self.dev_driver.take() else {
            self.phase = Phase::InGame(game);
            return;
        };
        rts_sim::perf::timed(perf.as_mut(), "dev_driver_enqueue", || {
            driver.enqueue_for_tick(&mut game)
        });
        let game_tick_start = StdInstant::now();
        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            game.tick_with_perf(perf.as_mut())
        }));
        if let Some(perf) = perf.as_mut() {
            perf.record_phase("game_tick", game_tick_start.elapsed());
        }
        let mut per_player_events: HashMap<u32, Vec<Event>> = match tick_result {
            Ok(events) => events.into_iter().collect(),
            Err(payload) => {
                let reason = panic_reason(&payload);
                dump_crash_replay(&self.room, &game, self.replay_start.as_ref(), &reason);
                self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
                self.phase = Phase::Lobby;
                self.dev_driver = None;
                self.dev_view_player_id = None;
                return;
            }
        };

        let tick_budget = self.current_tick_interval();
        let recipients = self.order.clone();
        let view_player_id = self.dev_view_player_id.unwrap_or(0);
        let observer_views = self.observer_views.clone();
        let full_vision_events = rts_sim::perf::timed(perf.as_mut(), "event_union", || {
            union_events(per_player_events.values())
        });
        let projection_policy = self.projection_policy();
        SnapshotFanout::new(
            &self.room,
            scheduler_lag,
            tick_budget,
            tick_start,
            &mut self.slow_tick_count,
            perf.as_mut(),
        )
        .send_to_recipients(&mut self.players, recipients, |id, player| {
            let role = if player.spectator {
                RecipientRole::Spectator
            } else {
                RecipientRole::ActivePlayer
            };
            let projection = if role == RecipientRole::Spectator {
                projection_policy.selected_perspective_snapshot_for(observer_view_or_all(
                    observer_views.get(&id),
                    &game,
                ))
            } else {
                projection_policy.dev_watch_snapshot_for(role, view_player_id)
            };
            let snapshot =
                projection.snapshot_with_events(&game, &mut per_player_events, &full_vision_events);
            Some(SnapshotFanoutPayload::new(snapshot, player.spectator))
        });

        self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
        self.dev_driver = Some(driver);
        self.phase = Phase::InGame(game);
    }
}
