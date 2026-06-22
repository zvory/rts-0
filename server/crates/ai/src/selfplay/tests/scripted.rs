use std::collections::BTreeMap;

use super::super::milestones::{CombatGoal, Milestones, PlayerMilestoneGoal};
use super::super::player_view::PlayerView;
use super::super::scripts::{
    MineOnlyScript, ProfileBackedScript, ScriptedPlayer, WorkerRushScript,
};
use super::harness::{finalize_self_play_success, replay_artifact_url, SelfPlayRunner};
use crate::config;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Snapshot, StartPayload};

#[test]
fn scripted_self_play_worker_rush_vs_economy() {
    if crate::skip_unless_full_ai("scripted_self_play_worker_rush_vs_economy") {
        return;
    }
    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Worker Rush".into(),
            color: "#e71d36".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Economy".into(),
            color: "#3a86ff".into(),
            is_ai: true,
        },
    ];
    let game = Game::new_without_ai_controllers(&players, 0x1234_5678);
    let start = game.start_payload();
    let specs = players.clone();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(WorkerRushScript::new(1, 2)),
        Box::new(ProfileBackedScript::economy_only(2)),
    ];
    let milestones = Milestones::with_goals(
        [
            (1, PlayerMilestoneGoal::default()),
            (2, PlayerMilestoneGoal::damaged_economy()),
        ],
        CombatGoal::worker_attack_by(1),
    );
    let mut runner = SelfPlayRunner::with_milestones(
        "scripted_self_play_worker_rush_vs_economy",
        game,
        start,
        specs,
        scripts,
        milestones,
    );

    match runner.run() {
        Ok(report) => finalize_self_play_success(&runner, &players, &report),
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    replay_artifact_url(&name)
                })
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("self-play failed: {}; REPLAY={artifact}", failure.reason);
        }
    }
}

/// A scripted player that does nothing but send idle workers to mine the nearest steel node.
/// No building, no training, no combat — pure passive mining.
///

#[test]
fn scripted_self_play_mine_only_steel_fairness() {
    if crate::skip_unless_full_ai("scripted_self_play_mine_only_steel_fairness") {
        return;
    }
    const TWO_MINUTES_TICKS: u32 = 2 * 60 * config::TICK_HZ;
    const STEEL_TOLERANCE: u32 = 15;

    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Miner A".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Miner B".into(),
            color: "#f72585".into(),
            is_ai: false,
        },
    ];
    let mut game = Game::new(&players, 0x1234_5678);
    let start = game.start_payload();

    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];

    let snapshots: BTreeMap<u32, Snapshot> = players
        .iter()
        .map(|p| (p.id, game.snapshot_for(p.id)))
        .collect();
    let alive_player_ids = game.alive_players();
    let mut commands = Vec::new();
    for script in &mut scripts {
        let pid = script.player_id();
        let Some(snapshot) = snapshots.get(&pid) else {
            continue;
        };
        let view = PlayerView {
            player_id: pid,
            tick: 0,
            start: &start,
            snapshot,
            alive_player_ids: &alive_player_ids,
        };
        commands.extend(
            script
                .commands(view)
                .into_iter()
                .map(|command| (pid, command)),
        );
    }
    for (player_id, command) in commands {
        game.enqueue(player_id, command);
    }

    for _ in 0..TWO_MINUTES_TICKS {
        game.tick();
    }

    let snap_a = game.snapshot_for(1);
    let snap_b = game.snapshot_for(2);

    let diff = snap_a.steel.abs_diff(snap_b.steel);

    assert!(
        diff <= STEEL_TOLERANCE,
        "after two minutes of passive mining, player 1 has {} steel and player 2 has {} steel (diff = {}, tolerance = {})",
        snap_a.steel,
        snap_b.steel,
        diff,
        STEEL_TOLERANCE
    );
}

/// Run a scripted match pair for a fixed number of ticks and assert both games expose identical
/// per-player snapshots before each tick.
#[cfg(test)]
fn assert_scripted_runs_identical_for_ticks(
    players: &[PlayerInit],
    scripts_a: &mut [Box<dyn ScriptedPlayer>],
    scripts_b: &mut [Box<dyn ScriptedPlayer>],
    start: &StartPayload,
    game_a: &mut Game,
    game_b: &mut Game,
    ticks: u32,
) {
    for tick in 0..ticks {
        let alive_a = game_a.alive_players();
        let alive_b = game_b.alive_players();
        let snapshots_a: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|p| (p.id, game_a.snapshot_for(p.id)))
            .collect();
        let snapshots_b: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|p| (p.id, game_b.snapshot_for(p.id)))
            .collect();
        for p in players {
            assert_eq!(
                snapshots_a[&p.id], snapshots_b[&p.id],
                "tick {tick}: player {} snapshots diverged between two fresh runs",
                p.id
            );
        }

        let mut commands_a = Vec::new();
        for script in scripts_a.iter_mut() {
            let pid = script.player_id();
            let Some(snapshot) = snapshots_a.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start,
                snapshot,
                alive_player_ids: &alive_a,
            };
            commands_a.extend(
                script
                    .commands(view)
                    .into_iter()
                    .map(|command| (pid, command)),
            );
        }
        let mut commands_b = Vec::new();
        for script in scripts_b.iter_mut() {
            let pid = script.player_id();
            let Some(snapshot) = snapshots_b.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start,
                snapshot,
                alive_player_ids: &alive_b,
            };
            commands_b.extend(
                script
                    .commands(view)
                    .into_iter()
                    .map(|command| (pid, command)),
            );
        }

        for (player_id, command) in commands_a {
            game_a.enqueue(player_id, command);
        }
        for (player_id, command) in commands_b {
            game_b.enqueue(player_id, command);
        }

        game_a.tick();
        game_b.tick();
    }
}

#[test]
fn identical_scripted_runs_are_identical() {
    const TICKS: u32 = 600;

    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "A".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "B".into(),
            color: "#f72585".into(),
            is_ai: false,
        },
    ];
    let mut game_a = Game::new(&players, 0x1234_5678);
    let start = game_a.start_payload();
    let mut scripts_a: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];
    let mut game_b = Game::new(&players, 0x1234_5678);
    let mut scripts_b: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];

    assert_scripted_runs_identical_for_ticks(
        &players,
        &mut scripts_a,
        &mut scripts_b,
        &start,
        &mut game_a,
        &mut game_b,
        TICKS,
    );

    // Command logs must also be identical.
    assert_eq!(
        game_a.command_log(),
        game_b.command_log(),
        "command logs diverged between two fresh runs"
    );
}
