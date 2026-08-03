#[path = "reference_strategy/strategy.rs"]
mod strategy;

use rts_ai::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::{Game, PlayerInit};
use strategy::ReferenceStrategy;

fn main() {
    let players = players();
    let mut game = Game::new_without_ai_controllers(&players, 0xA15D_0004);
    let mut controllers = vec![AiController::with_strategy(
        1,
        Box::new(ReferenceStrategy::default()),
    )];

    for _ in 0..36 {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
    }

    println!(
        "reference strategy produced {} replay-logged commands",
        game.command_log().len()
    );
    for entry in game.command_log() {
        println!(
            "tick={} player={} command={:?}",
            entry.tick, entry.player_id, entry.command
        );
    }
}

fn players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "SDK reference".to_string(),
            color: "#4cc9f0".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Passive opponent".to_string(),
            color: "#f72585".to_string(),
            is_ai: true,
        },
    ]
}
