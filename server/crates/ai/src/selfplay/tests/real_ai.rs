use super::super::replay::assert_replay_matches_live;
use super::harness::replay_artifact_url;
use crate::{AiController, AiThinkContext};
use rts_sim::game::replay::{EventLogEntry, ReplayStartComposition};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{kinds, Command as WireCommand, Event};

#[test]
fn real_ai_vs_real_ai() {
    use std::collections::{BTreeMap, BTreeSet};

    if crate::skip_unless_full_ai("real_ai_vs_real_ai") {
        return;
    }

    const MIN_PEAK_BARRACKS_ALIVE: usize = 1;
    const MIN_RIFLEMAN_TRAIN_COMMANDS: usize = 4;
    const MIN_SCOUT_CAR_TRAIN_COMMANDS: usize = 1;
    const MIN_TANK_TRAIN_COMMANDS: usize = 1;
    const MIN_ATTACK_MOVE_COMMANDS: usize = 4;
    const MIN_ATTACK_EVENTS: usize = 50;
    const TICKS: u32 = 13_824;

    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Alpha".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Beta".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ];
    let mut game = Game::new(&players, 0x1234_5678);
    let replay_start = ReplayStartComposition::capture(&game, super::server_build_sha())
        .expect("real-ai replay start should export");
    let mut controllers: Vec<AiController> = players
        .iter()
        .map(|player| AiController::new(player.id))
        .collect();

    let mut event_log = Vec::new();
    let mut max_barracks_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_riflemen_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_scout_cars_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_tanks_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut seen_riflemen: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut seen_scout_cars: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut seen_tanks: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut attack_events: BTreeMap<u32, usize> = BTreeMap::new();
    let mut death_events: BTreeMap<u32, usize> = BTreeMap::new();
    let mut barracks_build_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut rifleman_train_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut scout_car_train_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut tank_train_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut attack_move_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut command_log_cursor = 0usize;
    let panic_reason = |payload: &Box<dyn std::any::Any + Send>| -> String {
        if let Some(s) = payload.downcast_ref::<&'static str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic without string payload".to_string()
        }
    };
    let save_failure_artifact = |game: &Game, reason: &str| -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let artifact_name = format!("real_ai_vs_real_ai_failure_{ts}");
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("selfplay-failures")
            .join(&artifact_name);
        if std::fs::create_dir_all(&dir).is_ok() {
            let artifact = replay_start.finalize(game, None, game.scores());
            if let Ok(json) = serde_json::to_vec_pretty(&artifact) {
                let _ = std::fs::write(dir.join("replay.json"), json);
            }
        }
        let url = replay_artifact_url(&artifact_name);
        println!("REPLAY_ARTIFACT={artifact_name}");
        eprintln!("real_ai_vs_real_ai failure: {reason}");
        eprintln!("view replay: {url}");
        url
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for tick in 1..=TICKS {
            let start = game.start_payload();
            let alive_players = game.alive_players();
            let mut commands = Vec::new();
            for controller in &mut controllers {
                let player_id = controller.player_id();
                if !alive_players.contains(&player_id) {
                    continue;
                }
                let snapshot = game.snapshot_for(player_id);
                commands.extend(
                    controller
                        .think(AiThinkContext {
                            start: &start,
                            snapshot: &snapshot,
                            alive_player_ids: &alive_players,
                            retreat_commands: game.worker_retreat_commands_for(player_id),
                        })
                        .into_iter()
                        .map(|command| (player_id, command)),
                );
            }
            for (player_id, command) in commands {
                game.enqueue(player_id, command);
            }

            let tick_result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()));
            let tick_output = match tick_result {
                Ok(events) => events,
                Err(_) => {
                    let url = save_failure_artifact(&game, "Game::tick panicked");
                    panic!("real_ai_vs_real_ai: tick {tick} panicked; view replay: {url}");
                }
            };
            for (player_id, events) in tick_output {
                for event in events {
                    match event {
                        Event::Attack { .. } => {
                            *attack_events.entry(player_id).or_default() += 1;
                        }
                        Event::Death { .. } => {
                            *death_events.entry(player_id).or_default() += 1;
                        }
                        _ => {}
                    }
                    event_log.push(EventLogEntry {
                        tick,
                        player_id,
                        event,
                    });
                }
            }

            for player in &players {
                let snapshot = game.snapshot_for(player.id);
                let mut barracks_alive = 0usize;
                let mut riflemen_alive = 0usize;
                let mut scout_cars_alive = 0usize;
                let mut tanks_alive = 0usize;
                let seen_rifle = seen_riflemen.entry(player.id).or_default();
                let seen_scout = seen_scout_cars.entry(player.id).or_default();
                let seen_tank = seen_tanks.entry(player.id).or_default();
                for entity in snapshot.entities.iter().filter(|e| e.owner == player.id) {
                    if entity.kind == kinds::BARRACKS {
                        barracks_alive += 1;
                    }
                    if entity.kind == kinds::RIFLEMAN {
                        riflemen_alive += 1;
                        seen_rifle.insert(entity.id);
                    }
                    if entity.kind == kinds::SCOUT_CAR {
                        scout_cars_alive += 1;
                        seen_scout.insert(entity.id);
                    }
                    if entity.kind == kinds::TANK {
                        tanks_alive += 1;
                        seen_tank.insert(entity.id);
                    }
                }
                max_barracks_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(barracks_alive))
                    .or_insert(barracks_alive);
                max_riflemen_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(riflemen_alive))
                    .or_insert(riflemen_alive);
                max_scout_cars_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(scout_cars_alive))
                    .or_insert(scout_cars_alive);
                max_tanks_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(tanks_alive))
                    .or_insert(tanks_alive);
            }

            let command_log = game.command_log();
            for entry in &command_log[command_log_cursor..] {
                match &entry.command {
                    WireCommand::Build { building, .. } if building == kinds::BARRACKS => {
                        *barracks_build_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::Train { unit, .. } if unit == kinds::RIFLEMAN => {
                        *rifleman_train_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::Train { unit, .. } if unit == kinds::SCOUT_CAR => {
                        *scout_car_train_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::Train { unit, .. } if unit == kinds::TANK => {
                        *tank_train_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::AttackMove { .. } => {
                        *attack_move_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    _ => {}
                }
            }
            command_log_cursor = command_log.len();

            if players.iter().all(|player| {
                max_barracks_alive
                    .get(&player.id)
                    .copied()
                    .unwrap_or_default()
                    >= MIN_PEAK_BARRACKS_ALIVE
                    && rifleman_train_cmds
                        .get(&player.id)
                        .copied()
                        .unwrap_or_default()
                        >= MIN_RIFLEMAN_TRAIN_COMMANDS
                    && scout_car_train_cmds
                        .get(&player.id)
                        .copied()
                        .unwrap_or_default()
                        >= MIN_SCOUT_CAR_TRAIN_COMMANDS
                    && tank_train_cmds.get(&player.id).copied().unwrap_or_default()
                        >= MIN_TANK_TRAIN_COMMANDS
                    && attack_move_cmds
                        .get(&player.id)
                        .copied()
                        .unwrap_or_default()
                        >= MIN_ATTACK_MOVE_COMMANDS
                    && attack_events.get(&player.id).copied().unwrap_or_default()
                        >= MIN_ATTACK_EVENTS
            }) {
                break;
            }
        }

        for player in &players {
            let peak_barracks = max_barracks_alive
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let rifleman_trains = rifleman_train_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let attack_moves = attack_move_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let attacks = attack_events.get(&player.id).copied().unwrap_or_default();
            let seen_riflemen = seen_riflemen
                .get(&player.id)
                .map(|ids| ids.len())
                .unwrap_or_default();
            let scout_car_trains = scout_car_train_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let tank_trains = tank_train_cmds.get(&player.id).copied().unwrap_or_default();
            let seen_scout_cars = seen_scout_cars
                .get(&player.id)
                .map(|ids| ids.len())
                .unwrap_or_default();
            let seen_tanks = seen_tanks
                .get(&player.id)
                .map(|ids| ids.len())
                .unwrap_or_default();
            let peak_riflemen = max_riflemen_alive
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let peak_scout_cars = max_scout_cars_alive
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let peak_tanks = max_tanks_alive.get(&player.id).copied().unwrap_or_default();
            let barracks_builds = barracks_build_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();

            assert!(
                peak_barracks >= MIN_PEAK_BARRACKS_ALIVE,
                "player {} peaked at only {} live barracks (build cmds {}, train cmds {}, peak riflemen {}, seen riflemen {}, attack moves {}, attack events {})",
                player.id,
                peak_barracks,
                barracks_builds,
                rifleman_trains,
                peak_riflemen,
                seen_riflemen,
                attack_moves,
                attacks,
            );
            assert!(
                rifleman_trains >= MIN_RIFLEMAN_TRAIN_COMMANDS,
                "player {} trained only {} riflemen (peak barracks {}, peak riflemen {}, seen riflemen {}, attack moves {}, attack events {})",
                player.id,
                rifleman_trains,
                peak_barracks,
                peak_riflemen,
                seen_riflemen,
                attack_moves,
                attacks,
            );
            assert!(
                scout_car_trains >= MIN_SCOUT_CAR_TRAIN_COMMANDS,
                "player {} trained only {} scout cars (peak scout cars {}, seen scout cars {}, tank trains {}, peak tanks {}, seen tanks {}, attack moves {}, attack events {})",
                player.id,
                scout_car_trains,
                peak_scout_cars,
                seen_scout_cars,
                tank_trains,
                peak_tanks,
                seen_tanks,
                attack_moves,
                attacks,
            );
            assert!(
                tank_trains >= MIN_TANK_TRAIN_COMMANDS,
                "player {} trained only {} tanks (peak tanks {}, seen tanks {}, scout car trains {}, peak scout cars {}, seen scout cars {}, attack moves {}, attack events {})",
                player.id,
                tank_trains,
                peak_tanks,
                seen_tanks,
                scout_car_trains,
                peak_scout_cars,
                seen_scout_cars,
                attack_moves,
                attacks,
            );
            assert!(
                attack_moves >= MIN_ATTACK_MOVE_COMMANDS,
                "player {} issued only {} attack-move commands (peak barracks {}, rifleman train cmds {}, scout car train cmds {}, tank train cmds {}, peak riflemen {}, peak scout cars {}, peak tanks {}, attack events {})",
                player.id,
                attack_moves,
                peak_barracks,
                rifleman_trains,
                scout_car_trains,
                tank_trains,
                peak_riflemen,
                peak_scout_cars,
                peak_tanks,
                attacks,
            );
            assert!(
                attacks >= MIN_ATTACK_EVENTS,
                "player {} produced only {} attack events (peak barracks {}, rifleman train cmds {}, scout car train cmds {}, tank train cmds {}, attack moves {}, peak riflemen {}, peak scout cars {}, peak tanks {}, seen riflemen {}, seen scout cars {}, seen tanks {}, deaths {})",
                player.id,
                attacks,
                peak_barracks,
                rifleman_trains,
                scout_car_trains,
                tank_trains,
                attack_moves,
                peak_riflemen,
                peak_scout_cars,
                peak_tanks,
                seen_riflemen,
                seen_scout_cars,
                seen_tanks,
                death_events.get(&player.id).copied().unwrap_or_default(),
            );
        }

        assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(|failure| {
            panic!("AI vs AI replay determinism failed: {}", failure.reason);
        });
    }));

    if let Err(payload) = result {
        let reason = panic_reason(&payload);
        let url = save_failure_artifact(&game, &reason);
        panic!("real_ai_vs_real_ai failed; view replay: {url}");
    }

    // Write a replay artifact so the neutral replay artifact viewer can load it.
    let artifact = replay_start.finalize(&game, None, game.scores());
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let artifact_name = format!("real_ai_vs_real_ai_{ts}");
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("selfplay-artifacts")
        .join(&artifact_name);
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::to_vec_pretty(&artifact).unwrap();
    std::fs::write(dir.join("replay.json"), json).unwrap();
    println!("REPLAY_ARTIFACT={artifact_name}");
}
