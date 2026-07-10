use super::player_view::PlayerView;
use super::scripts::{ProfileBackedScript, ScriptedPlayer};
use crate::ai_core::profiles::{AI_2_1_ID, AI_TURTLE_ID};
use rts_sim::game::{Game, PlayerInit};

pub struct LiveSelfPlay {
    players: Vec<PlayerInit>,
    scripts: Vec<Box<dyn ScriptedPlayer>>,
}

impl LiveSelfPlay {
    pub fn default_match() -> Self {
        let players = vec![
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "Alpha Script".to_string(),
                color: "#6f8fa8".to_string(),
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "Bravo Script".to_string(),
                color: "#b2775f".to_string(),
                is_ai: true,
            },
        ];
        let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
            Box::new(ProfileBackedScript::new(players[0].id, AI_2_1_ID)),
            Box::new(ProfileBackedScript::new(players[1].id, AI_TURTLE_ID)),
        ];
        Self { players, scripts }
    }

    pub fn players(&self) -> &[PlayerInit] {
        &self.players
    }

    pub fn enqueue_for_tick(&mut self, game: &mut Game) {
        let tick = game.tick_count();
        let start = game.start_payload();
        let alive_player_ids = game.alive_players();
        let mut commands = Vec::new();
        for script in &mut self.scripts {
            let player_id = script.player_id();
            let snapshot = game.snapshot_for(player_id);
            let view = PlayerView {
                player_id,
                tick,
                start: &start,
                snapshot: &snapshot,
                alive_player_ids: &alive_player_ids,
            };
            for command in script.commands(view) {
                commands.push((player_id, command));
            }
        }
        for (player_id, command) in commands {
            game.enqueue(player_id, command);
        }
    }
}
