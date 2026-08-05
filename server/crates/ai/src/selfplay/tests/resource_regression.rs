use std::collections::BTreeMap;

use super::super::player_view::{is_complete, kind_of, PlayerView};
use super::super::scripts::{ProfileBackedScript, ScriptedPlayer};
use crate::config;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{EntityView, Snapshot, StartPayload};

#[derive(Default)]
struct ResourceRegressionEvidence {
    first_expansion_pump_jack_tick: Option<u32>,
    first_second_completed_resource_depot_tick: Option<u32>,
}

fn profile_players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Resource Regression".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Mirror".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ]
}

fn completed_resource_depots(snapshot: &Snapshot, player_id: u32) -> Vec<&EntityView> {
    snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::ResourceDepot))
        .filter(|entity| is_complete(entity))
        .collect()
}

fn has_pump_jack_for_expansion_depot(
    start: &StartPayload,
    snapshot: &Snapshot,
    player_id: u32,
    starting_depot_id: u32,
) -> bool {
    let range_px = config::MINING_ANCHOR_RANGE_TILES * start.map.tile_size as f32;
    let range2 = range_px * range_px + 0.01;
    let expansion_depots = completed_resource_depots(snapshot, player_id)
        .into_iter()
        .filter(|depot| depot.id != starting_depot_id);
    let pump_jacks = snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::PumpJack))
        .collect::<Vec<_>>();

    expansion_depots.into_iter().any(|depot| {
        pump_jacks.iter().any(|pump_jack| {
            let dx = depot.x - pump_jack.x;
            let dy = depot.y - pump_jack.y;
            dx * dx + dy * dy <= range2
        })
    })
}

fn run_resource_regression_profile(max_ticks: u32) -> ResourceRegressionEvidence {
    let players = profile_players();
    let mut game = Game::new_without_ai_controllers(&players, 0x4100_0004);
    let start = game.start_payload();
    let starting_depot_id = game
        .snapshot_for(1)
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && kind_of(entity) == Some(EntityKind::ResourceDepot))
        .map(|entity| entity.id)
        .expect("player one should start with a Resource Depot");
    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(ProfileBackedScript::economy_only(1)),
        Box::new(ProfileBackedScript::economy_only(2)),
    ];
    let mut evidence = ResourceRegressionEvidence::default();

    for tick in 0..max_ticks {
        let alive_player_ids = game.alive_players();
        let snapshots: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|player| (player.id, game.snapshot_for(player.id)))
            .collect();
        let player_one_snapshot = &snapshots[&1];
        if evidence
            .first_second_completed_resource_depot_tick
            .is_none()
            && completed_resource_depots(player_one_snapshot, 1).len() >= 2
        {
            evidence.first_second_completed_resource_depot_tick = Some(tick);
        }
        if evidence.first_expansion_pump_jack_tick.is_none()
            && has_pump_jack_for_expansion_depot(&start, player_one_snapshot, 1, starting_depot_id)
        {
            evidence.first_expansion_pump_jack_tick = Some(tick);
        }

        let mut commands = Vec::new();
        for script in &mut scripts {
            let pid = script.player_id();
            let Some(snapshot) = snapshots.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start: &start,
                snapshot,
                alive_player_ids: &alive_player_ids,
            };
            commands.extend(
                script
                    .commands(view, game.worker_retreat_commands_for(pid))
                    .into_iter()
                    .map(|command| (pid, command)),
            );
        }

        for (player_id, command) in commands {
            if player_id == 1 {
                if let Command::Gather { node, .. } = &command {
                    let kind = start
                        .map
                        .resources
                        .iter()
                        .find(|resource| resource.id == *node)
                        .and_then(|resource| resource.kind.parse().ok());
                    assert_ne!(
                        kind,
                        Some(EntityKind::Oil),
                        "oil at tick {tick} should use the Depot's automatic Pump Jack job, not direct gather"
                    );
                }
                assert!(
                    !matches!(
                        command,
                        Command::Build {
                            building: EntityKind::PumpJack,
                            ..
                        }
                    ),
                    "Pump Jack at tick {tick} should come from the Depot's automatic job"
                );
            }
            game.enqueue(player_id, command);
        }

        game.tick();
    }

    evidence
}

#[test]
fn profile_backed_expansion_receives_automatic_pump_jack() {
    if crate::skip_unless_full_ai("profile_backed_expansion_receives_automatic_pump_jack") {
        return;
    }
    let evidence = run_resource_regression_profile(9_000);

    assert!(
        evidence
            .first_second_completed_resource_depot_tick
            .is_some(),
        "expected AI 1.0 economy progression to complete an expansion Resource Depot"
    );
    assert!(
        evidence.first_expansion_pump_jack_tick.is_some(),
        "expected the completed expansion Resource Depot to start an automatic Pump Jack"
    );
    assert!(
        evidence.first_expansion_pump_jack_tick
            >= evidence.first_second_completed_resource_depot_tick,
        "the expansion Pump Jack must not precede its completed Resource Depot"
    );
}
