use std::collections::{BTreeMap, BTreeSet};

use crate::{AiController, AiThinkContext};
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Command as WireCommand, Event};

#[test]
fn live_ai_two_vs_two_keeps_allied_controllers_independent_and_non_hostile() {
    if crate::skip_unless_full_ai(
        "live_ai_two_vs_two_keeps_allied_controllers_independent_and_non_hostile",
    ) {
        return;
    }
    const TICKS: u32 = 3_600;

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
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Bravo".into(),
            color: "#4895ef".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 3,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Charlie".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 4,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Delta".into(),
            color: "#b5179e".into(),
            is_ai: true,
        },
    ];
    let mut game = Game::new_with_starting_resources(&players, 10_000, 10_000, 0x715A_F311);
    let mut controllers: Vec<AiController> = players
        .iter()
        .map(|player| AiController::new(player.id))
        .collect();
    let mut entity_owner: BTreeMap<u32, u32> = BTreeMap::new();
    let mut command_players = BTreeSet::new();
    let mut attack_command_players = BTreeSet::new();
    let mut command_log_cursor = 0usize;

    for _ in 0..TICKS {
        let start = game.start_payload();
        let alive_players = game.alive_players();
        let mut commands = Vec::new();
        for controller in &mut controllers {
            let player_id = controller.player_id();
            if !alive_players.contains(&player_id) {
                continue;
            }
            let snapshot = game.snapshot_for(player_id);
            for entity in &snapshot.entities {
                if entity.owner != 0 {
                    entity_owner.insert(entity.id, entity.owner);
                }
            }
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
            if let Command::Attack { target, .. } = &command {
                if let Some(target_owner) = entity_owner.get(target) {
                    assert!(
                        game.is_enemy_player(player_id, *target_owner),
                        "AI player {player_id} issued direct attack against allied player {target_owner}"
                    );
                }
            }
            game.enqueue(player_id, command);
        }

        let tick_events = game.tick();
        for (recipient, events) in tick_events {
            for event in events {
                if let Event::Attack { from, to, .. } = event {
                    let attacker_owner = entity_owner.get(&from).copied();
                    let target_owner = entity_owner.get(&to).copied();
                    if let (Some(attacker_owner), Some(target_owner)) =
                        (attacker_owner, target_owner)
                    {
                        assert!(
                            game.is_enemy_player(attacker_owner, target_owner),
                            "same-team attack event delivered to player {recipient}: {attacker_owner} attacked {target_owner}"
                        );
                    }
                }
            }
        }

        for player in &players {
            let snapshot = game.snapshot_for(player.id);
            for entity in &snapshot.entities {
                if entity.owner != 0 {
                    entity_owner.insert(entity.id, entity.owner);
                }
            }
        }

        let command_log = game.command_log();
        for entry in &command_log[command_log_cursor..] {
            command_players.insert(entry.player_id);
            match &entry.command {
                WireCommand::Attack { target, .. } => {
                    attack_command_players.insert(entry.player_id);
                    let target_owner = entity_owner.get(target).copied().unwrap_or_default();
                    assert!(
                        target_owner == 0 || game.is_enemy_player(entry.player_id, target_owner),
                        "AI player {} recorded direct attack against non-enemy owner {}",
                        entry.player_id,
                        target_owner
                    );
                }
                WireCommand::AttackMove { .. } => {
                    attack_command_players.insert(entry.player_id);
                }
                _ => {}
            }
        }
        command_log_cursor = command_log.len();

        if command_players.len() == players.len() && attack_command_players.len() >= 2 {
            break;
        }
    }

    assert_eq!(
        command_players,
        players
            .iter()
            .map(|player| player.id)
            .collect::<BTreeSet<_>>(),
        "each AI player should own and issue its own commands"
    );
    assert!(
        !attack_command_players.is_empty(),
        "short 2v2 AI run should reach at least one attack intent"
    );
}
